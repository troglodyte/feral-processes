//! The `impl Game` shell for bounty contracts: gather what the roll needs,
//! keep exactly one quarry standing for a held bounty, and answer the
//! compass. The decisions are `bounty::roll` and `bounty::reconcile`; this
//! file only reads the world and writes the result back (ADR 0044).

use bevy_ecs::prelude::*;
use rand::prelude::*;

use crate::Game;
use crate::bounty::{self, BountyInput, BountyQuarry};
use crate::components::{BountyMark, Position, Stats};
use crate::contracts::{ContractDef, ContractId, Objective};
use crate::game::contracts::fold;
use crate::game::spawning::SpawnPins;
use crate::resources::{ActiveContracts, CompassBearing, ZoneLevel};
use crate::settlements::CompassTarget;
use crate::tuning::*;

impl Game {
    /// The bounty the run holds, if any. At most one is ever held: the board
    /// offers none while one is.
    pub(crate) fn held_bounty(&self) -> Option<(ContractId, BountyQuarry)> {
        self.world
            .resource::<ActiveContracts>()
            .active
            .iter()
            .find_map(|c| match &c.def.objective {
                Objective::Bounty(quarry) => Some((c.def.id.clone(), quarry.clone())),
                _ => None,
            })
    }

    /// The bounty this epoch's Broker board adds, if its roll came up.
    ///
    /// **A separate RNG salted off the board seed**, never a draw on the
    /// board's own stream and never an extra entry in the pool the board
    /// draws from: either would shift which of the other contracts every
    /// existing board offers. The odds draw is made first and always, so the
    /// failed-id prune below can tell "no bounty this epoch" from "the same
    /// bounty again".
    pub(crate) fn board_bounty(&mut self, seed: u64) -> Option<ContractDef> {
        let zone = self.world.resource::<ZoneLevel>().0;
        if zone < BOUNTY_MIN_ZONE || self.held_bounty().is_some() {
            return None;
        }
        let mut rng = StdRng::seed_from_u64(fold(seed, b"bounty"));
        if !rng.random_bool(BOUNTY_BOARD_ODDS) {
            self.world.resource_mut::<ActiveContracts>().failed.clear();
            return None;
        }
        let input = self.bounty_input(zone)?;
        let def = bounty::roll(&input, &mut rng)?.def;
        // A failed id only matters while this epoch's roll is still it.
        self.world
            .resource_mut::<ActiveContracts>()
            .failed
            .retain(|id| *id == def.id);
        self.offerable(&def).then_some(def)
    }

    /// What the sector can supply a bounty roll with.
    ///
    /// Measured from the **base's anchor**, not the player: the board is
    /// derived and must read the same wherever the party stands, and the
    /// Broker is on the base anyway. The candidates are a fixed lattice
    /// (`bounty::anchor_samples`) filtered by terrain and by where the seed
    /// says settlements stand, which is why a settlement the party has not
    /// found yet still keeps a quarry off its tile.
    fn bounty_input(&mut self, zone: u32) -> Option<BountyInput> {
        let home = self.anchor_position()?;
        let habitat: Vec<String> = self
            .template_pools()
            .species
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        if habitat.is_empty() {
            return None;
        }
        let links: Vec<(i32, i32)> = {
            let mut q = self
                .world
                .query_filtered::<&Position, With<crate::components::SurfaceLink>>();
            q.iter(&self.world).map(|p| (p.x, p.y)).collect()
        };
        let mut anchor_candidates = Vec::new();
        for (x, y) in bounty::anchor_samples(home) {
            let tile = self.ground_tile(x, y);
            if tile.walkable
                && tile.open_to_hostiles()
                && !links.contains(&(x, y))
                && !self.near_derived_settlement(x, y)
            {
                anchor_candidates.push((x, y));
            }
        }
        Some(BountyInput {
            zone,
            habitat,
            anchor_candidates,
        })
    }

    /// Whether the seed puts a settlement within footprint-and-walk-out
    /// reach of `(x, y)`. Checks the regions the clearance box touches.
    fn near_derived_settlement(&self, x: i32, y: i32) -> bool {
        let reach = SETTLEMENT_FOOTPRINT_MAX_RADIUS + BOUNTY_SETTLEMENT_MARGIN;
        let seed = self.world.resource::<crate::world::WorldMap>().seed();
        let db = self.world.resource::<crate::settlements::SettlementDb>();
        let lo = crate::settlements::placement::region_of(x - reach, y - reach);
        let hi = crate::settlements::placement::region_of(x + reach, y + reach);
        (lo.ry..=hi.ry)
            .flat_map(|ry| {
                (lo.rx..=hi.rx).map(move |rx| crate::settlements::SettlementKey { rx, ry })
            })
            .filter_map(|key| crate::settlements::settlement_at(seed, db, key))
            .any(|p| (p.cell.0 - x).abs().max((p.cell.1 - y).abs()) <= reach)
    }

    /// Keeps exactly one quarry standing for the held bounty, and none
    /// otherwise. Idempotent, and cheap enough for every tick.
    ///
    /// This one check is every survival rule: a load, a breach and a cull
    /// all leave the map without its quarry for a moment, and none of them
    /// needs its own repair. The marks are read in `Entity` order so which
    /// of two duplicates is kept never depends on query order.
    ///
    /// Skipped mid-fight: a quarry that falls in a bounty battle is despawned
    /// by the fight's own teardown, and respawning it under the player's feet
    /// before that runs would be a second quarry.
    pub(crate) fn ensure_bounty_quarry(&mut self) {
        if self.has_active_battle() {
            return;
        }
        let held = self.held_bounty();
        let mut marks: Vec<(Entity, ContractId)> = {
            let mut q = self.world.query::<(Entity, &BountyMark)>();
            q.iter(&self.world)
                .map(|(e, m)| (e, m.contract.clone()))
                .collect()
        };
        marks.sort();
        let ids: Vec<ContractId> = marks.iter().map(|(_, id)| id.clone()).collect();
        let plan = bounty::reconcile(held.as_ref().map(|(id, _)| id), &ids);
        for i in plan.despawn {
            self.world.despawn(marks[i].0);
        }
        if let Some((id, quarry)) = &held
            && plan.spawn
        {
            self.spawn_bounty_quarry(id, quarry);
        }
        // A selection naming a contract that is gone points at nothing.
        let selected = self.world.resource::<CompassBearing>().0.clone();
        if let Some(CompassTarget::Bounty(id)) = selected
            && held.as_ref().is_none_or(|(held_id, _)| *held_id != id)
        {
            self.set_compass_bearing(None);
        }
    }

    /// Puts the quarry on the map at its anchor, at the grade the contract
    /// fixed when it was rolled.
    ///
    /// Spawned through the boss path (`boss: true`, which also keeps
    /// `gather_pack` from sweeping it into another pack) with its rarity
    /// pinned, then scaled by the grade. The grade is applied to the baked
    /// stats afterwards rather than threaded through the spawn: it is the one
    /// thing no other spawn has, and `Stats` is what a fight reads.
    fn spawn_bounty_quarry(&mut self, id: &ContractId, quarry: &BountyQuarry) -> Option<Entity> {
        let pins = SpawnPins {
            rarity: Some(quarry.rarity),
            routines: None,
        };
        let entity = self.spawn_wild_creature_pinned(
            &quarry.species,
            quarry.anchor.0,
            quarry.anchor.1,
            1.0,
            true,
            pins,
        )?;
        let grade = quarry.grade();
        if let Some(mut stats) = self.world.get_mut::<Stats>(entity) {
            stats.max_hp = (stats.max_hp as f32 * grade).round() as i32;
            stats.hp = stats.max_hp;
            stats.atk = (stats.atk as f32 * grade).round() as i32;
        }
        self.world.entity_mut(entity).insert(BountyMark {
            contract: id.clone(),
            anchor: quarry.anchor,
            tether: quarry.tether,
        });
        Some(entity)
    }

    /// Takes a bounty's quarry off the map and out of the compass. For a
    /// contract that has ended without the tick having noticed yet.
    pub(crate) fn drop_bounty_quarry(&mut self, id: &ContractId) {
        let doomed: Vec<Entity> = {
            let mut q = self.world.query::<(Entity, &BountyMark)>();
            q.iter(&self.world)
                .filter(|(_, m)| m.contract == *id)
                .map(|(e, _)| e)
                .collect()
        };
        for e in doomed {
            self.world.despawn(e);
        }
        if self.world.resource::<CompassBearing>().0 == Some(CompassTarget::Bounty(id.clone())) {
            self.set_compass_bearing(None);
        }
    }

    /// The compass row for the held bounty, from the live quarry's position.
    pub(crate) fn bounty_compass_target(&mut self) -> Option<(CompassTarget, String, (i32, i32))> {
        let (id, quarry) = self.held_bounty()?;
        let mut q = self.world.query::<(&BountyMark, &Position)>();
        let tile = q
            .iter(&self.world)
            .find(|(m, _)| m.contract == id)
            .map(|(_, p)| (p.x, p.y))?;
        Some((
            CompassTarget::Bounty(id),
            format!("Bounty: {}", quarry.name),
            tile,
        ))
    }
}
