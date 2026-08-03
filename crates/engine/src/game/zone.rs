//! The zone the player currently stands in: locating things on the map,
//! stamping the base platform, and stepping through a portal to the next
//! zone.

use crate::tuning::{INITIAL_WILD_POPULATION, MAX_BUILD_DISTANCE_FROM_HOME, STACK_LINKS_PER_ZONE};
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
        let player = self.player_entity();
        let label = self.entity_label(nest);
        let dmg = battle::compute_damage(self.effective_atk(player), 0, 5) as u32;
        let Some(mut durability) = self.world.get_mut::<Durability>(nest) else {
            return;
        };
        durability.hp = durability.hp.saturating_sub(dmg);
        let destroyed = durability.hp == 0;
        if destroyed {
            self.log(format!("The {label} crashes and collapses!"));
            self.despawn_nest(nest);
        } else {
            self.log(format!(
                "You unleash a data strike into the {label} for {dmg} damage."
            ));
        }
    }

    /// Despawns `nest`, first stripping `NestGuardian` from every creature
    /// tethered to it so none is left pointing at a dead entity — they
    /// resume ordinary wandering. Despawning implicitly cancels anything
    /// left in `Nest::pending_respawns`.
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
            self.world.entity_mut(guardian).remove::<NestGuardian>();
        }
        self.world.despawn(nest);
    }

    /// Stamps the base platform centered on `(cx, cy)`: every tile within
    /// `MAX_BUILD_DISTANCE_FROM_HOME` (Chebyshev) becomes walkable
    /// `Biome::Platform`, and every hostile and nest standing inside is
    /// obliterated. Deploying a Home and breaching into a new zone are the
    /// only callers.
    pub(crate) fn stamp_platform(&mut self, cx: i32, cy: i32) {
        {
            let mut map = self.world.resource_mut::<WorldMap>();
            for dy in -MAX_BUILD_DISTANCE_FROM_HOME..=MAX_BUILD_DISTANCE_FROM_HOME {
                for dx in -MAX_BUILD_DISTANCE_FROM_HOME..=MAX_BUILD_DISTANCE_FROM_HOME {
                    map.set_override(
                        cx + dx,
                        cy + dy,
                        Tile {
                            biome: Biome::Platform,
                            walkable: true,
                        },
                    );
                }
            }
        }

        let inside = |p: &Position| {
            (p.x - cx).abs() <= MAX_BUILD_DISTANCE_FROM_HOME
                && (p.y - cy).abs() <= MAX_BUILD_DISTANCE_FROM_HOME
        };
        let hostiles: Vec<Entity> = {
            let mut query = self
                .world
                .query_filtered::<(Entity, &Position), With<Hostile>>();
            query
                .iter(&self.world)
                .filter(|(_, p)| inside(p))
                .map(|(e, _)| e)
                .collect()
        };
        for e in hostiles {
            self.world.despawn(e);
        }
        // Nests route through despawn_nest rather than a bare despawn: a
        // guardian can be standing outside the slab while its nest is
        // inside it, and would otherwise be left tethered to a dead entity.
        let nests: Vec<Entity> = {
            let mut query = self
                .world
                .query_filtered::<(Entity, &Position), With<Nest>>();
            query
                .iter(&self.world)
                .filter(|(_, p)| inside(p))
                .map(|(e, _)| e)
                .collect()
        };
        for nest in nests {
            self.despawn_nest(nest);
        }
        // A Stack link under the slab is unreachable — nothing spawns on
        // platform floor and the base's own structures sit on it — so it goes
        // the way of the hostiles above. This matters for the Home-deployment
        // caller only: on a breach the platform is stamped before
        // `spawn_surface_links` runs, and that skips `Biome::Platform`
        // outright. It is not a rare collision, which is why it is worth
        // sweeping rather than trusting placement: `STACK_NEAREST_LINK_TILES`
        // puts a zone's first link 5-8 tiles from where the player arrives,
        // against a slab of `MAX_BUILD_DISTANCE_FROM_HOME` 7.
        let links: Vec<Entity> = {
            let mut query = self
                .world
                .query_filtered::<(Entity, &Position), With<SurfaceLink>>();
            query
                .iter(&self.world)
                .filter(|(_, p)| inside(p))
                .map(|(e, _)| e)
                .collect()
        };
        for e in links {
            self.world.despawn(e);
        }

        self.world.resource_mut::<Platform>().center = Some((cx, cy));
    }

    /// Removes the platform slab, restoring natural terrain underneath.
    /// Called when the Home is demolished — the slab is defined as
    /// "centered on the current Home", so no Home means no slab.
    pub(crate) fn clear_platform(&mut self) {
        let Some((cx, cy)) = self.world.resource::<Platform>().center else {
            return;
        };
        {
            let mut map = self.world.resource_mut::<WorldMap>();
            for dy in -MAX_BUILD_DISTANCE_FROM_HOME..=MAX_BUILD_DISTANCE_FROM_HOME {
                for dx in -MAX_BUILD_DISTANCE_FROM_HOME..=MAX_BUILD_DISTANCE_FROM_HOME {
                    map.clear_override(cx + dx, cy + dy);
                }
            }
        }
        self.world.resource_mut::<Platform>().center = None;
    }

    pub(crate) fn find_blocking_structure_at(&mut self, x: i32, y: i32) -> Option<Entity> {
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
    /// `(x, y)`, if any — checked before the generic blocking-structure
    /// check in `move_player` so walking onto one breaches the zone instead
    /// of just bumping into it.
    pub(crate) fn find_zone_portal_at(&mut self, x: i32, y: i32) -> Option<Entity> {
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
        // Breaching does not despawn structures — the base travels — so
        // anything zone-local has to be named here or it comes along at its
        // old coordinates. A `SurfaceLink` is zone-local: it opens onto a
        // frame generated for the sector it stands in, and left alive it
        // would ride the breach and could land inside the newly stamped base
        // platform.
        let stale: Vec<Entity> = {
            let mut query = self
                .world
                .query_filtered::<Entity, Or<(With<Hostile>, With<Nest>, With<SurfaceLink>)>>();
            query.iter(&self.world).collect()
        };
        for e in stale {
            self.world.despawn(e);
        }

        // Snapshot each structure's offset from the Home before the map is
        // swapped, so the base can be rebuilt in the same layout around the
        // new spawn point. No Home means there's no base to carry — every
        // structure is left behind instead, the way all of them used to be,
        // and any cronjob pointing at one is dropped rather than left
        // dangling. Unreachable through the public API (a Portal can't be
        // built without a Home, and demolishing a Home cascades to the
        // Portal), but cheaper to handle than to prove impossible.
        let home = self.home_position();
        let offsets: Vec<(Entity, (i32, i32))> = match home {
            Some(home) => {
                let mut query = self
                    .world
                    .query_filtered::<(Entity, &Position), With<Structure>>();
                query
                    .iter(&self.world)
                    .map(|(e, p)| (e, (p.x - home.x, p.y - home.y)))
                    .collect()
            }
            None => {
                let orphans: Vec<Entity> = {
                    let mut query = self.world.query_filtered::<Entity, With<Structure>>();
                    query.iter(&self.world).collect()
                };
                for e in orphans {
                    self.world.despawn(e);
                }
                let dangling: Vec<Entity> = {
                    let mut query = self.world.query_filtered::<Entity, With<Task>>();
                    query.iter(&self.world).collect()
                };
                for e in dangling {
                    self.world.entity_mut(e).remove::<Task>();
                }
                Vec::new()
            }
        };

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
        let mut new_map = WorldMap::new(new_seed);
        let start = find_walkable_start(&mut new_map);
        self.world.insert_resource(new_map);
        self.world.insert_resource(ZoneSpawnPoint {
            x: start.0,
            y: start.1,
        });

        for (e, (dx, dy)) in offsets {
            if let Some(mut pos) = self.world.get_mut::<Position>(e) {
                pos.x = start.0 + dx;
                pos.y = start.1 + dy;
            }
        }
        // The departed zone's slab went with the old WorldMap — a freshly
        // generated one starts with an empty override overlay — so there's
        // nothing to clean up and only ever one slab in existence.
        if home.is_some() {
            self.stamp_platform(start.0, start.1);
        }

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
        // Credits can buy a Portal Fragment on the far side, so in principle
        // a stockpile could fund a breach out of a zone it never worked. That
        // is priced rather than forbidden: at a 1-Credit sell rate and 8
        // Credits a fragment, skipping zone 3 means having sold 160 items
        // into a cargo cap first, which is a zone's work either way. Closing
        // it instead would mean pulling Portal Fragments off every trader,
        // and that listing is the only route from base production to
        // progression — see `balance_sim::ticks_to_afford_portal`.
        // Every trader's shelf goes with it, for the same reason: a shelf
        // holding a zone's worth of salvage is precisely the stockpile that
        // wipe exists to strand. Cleared explicitly rather than left to rot —
        // the base travels, so a stale entry keyed on a tile nobody stands on
        // any more would still be saved, and would spring back the day a
        // trader was rebuilt on the matching tile.
        self.world.insert_resource(BuybackLedger::default());

        // Same reason, and the same trap: a zone's links are gone, but their
        // maps are keyed by tile and would otherwise draw the last sector's
        // walked corridors onto a fresh link that happens to land on a
        // matching coordinate.
        self.world.insert_resource(StackMemory::default());

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
        self.spawn_initial_creatures(INITIAL_WILD_POPULATION);
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
