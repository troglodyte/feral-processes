//! The settlement footprint's map entities: the one writer, and what
//! happens to whatever already stood where a footprint grows.
//!
//! `game/settlement_growth.rs` holds what a town *is* — kind, vitality, the
//! growth latch and the radius those derive; this holds the square that
//! radius draws on the map, the door that keeps the map matching it, and
//! displacement, since both are about the footprint's *entities* rather
//! than about what the town is.

use bevy_ecs::prelude::{Entity, With};
use std::collections::HashSet;

use crate::Game;
use crate::components::{
    Caravan, Glyph, GlyphColor, Position, Settlement, SettlementCentre, SurfaceLink,
};
use crate::resources::{AnchorEntity, MessageKind, Outposts};
use crate::settlements::{SettlementKey, SettlementKind};

impl Game {
    /// Every cell `key`'s footprint covers right now, centred on its
    /// resolved tile — the one derivation every reader below calls rather
    /// than each re-deriving the square from a radius of its own.
    ///
    /// `Vec::new()` for an unmaterialized key, `settlement_shelf`'s reason:
    /// nothing here may panic on a key `resources::Settlements` does not
    /// know.
    pub(crate) fn footprint(&self, key: SettlementKey) -> Vec<(i32, i32)> {
        let Some(tile) = self
            .world
            .resource::<crate::resources::Settlements>()
            .0
            .get(&key)
            .map(|known| known.tile)
        else {
            return Vec::new();
        };
        let Some(radius) = self.settlement_radius(key) else {
            return Vec::new();
        };
        let side = (2 * radius + 1) as usize;
        let mut cells = Vec::with_capacity(side * side);
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                cells.push((tile.0 + dx, tile.1 + dy));
            }
        }
        cells
    }

    /// **The one writer of a settlement's map entities.** Diffs the cells
    /// `footprint` says should exist against the ones that do, despawns and
    /// spawns to match, and repaints every surviving cell's glyph to what
    /// `settlement_kind` says the town is now.
    ///
    /// **Early return when the cell count and the centre's glyph already
    /// match, and nothing is stranded** — this runs once a tick for every
    /// known town (beside `latch_growth`, which only fires on a flip)
    /// whether or not anything moved, and re-spawning cells that already
    /// exist would repaint the map every tick for no reason.
    ///
    /// **The stranded check is the deferral trap's other half.** A Stack
    /// entrance skipped by `displace` because the party stood inside it
    /// sits under a cell that is no longer "newly covered" on the very next
    /// tick — the settlement entity was already spawned there — so without
    /// this, the early return above would fire forever and the entrance
    /// would never relocate once the party surfaced. The check is one query
    /// over every `SurfaceLink` filtered against a footprint of at most 49
    /// cells, cheap enough to pay every tick this would otherwise no-op.
    ///
    /// The centre is the cell at the settlement's own tile and only that one
    /// keeps `SettlementCentre` — the radius is never below 1, so the centre
    /// is never among the cells a shrink despawns, which is what lets a
    /// tether stay pointed at one entity through a resize.
    pub(crate) fn sync_settlement_footprint(&mut self, key: SettlementKey) {
        let Some(tile) = self
            .world
            .resource::<crate::resources::Settlements>()
            .0
            .get(&key)
            .map(|known| known.tile)
        else {
            return;
        };
        let Some(glyph) = self.settlement_kind(key).map(SettlementKind::glyph) else {
            return;
        };
        let wanted = self.footprint(key);
        let want: HashSet<(i32, i32)> = wanted.iter().copied().collect();

        let existing: Vec<(Entity, (i32, i32), bool)> = {
            let mut query = self
                .world
                .query::<(Entity, &Settlement, &Position, Option<&SettlementCentre>)>();
            query
                .iter(&self.world)
                .filter(|(_, settlement, ..)| settlement.key == key)
                .map(|(entity, _, pos, centre)| (entity, (pos.x, pos.y), centre.is_some()))
                .collect()
        };
        let centre_glyph = existing
            .iter()
            .find(|(_, _, is_centre)| *is_centre)
            .and_then(|(entity, ..)| self.world.get::<Glyph>(*entity))
            .map(|drawn| drawn.ch);

        let stranded: Vec<(i32, i32)> = {
            let mut query = self.world.query_filtered::<&Position, With<SurfaceLink>>();
            query
                .iter(&self.world)
                .map(|pos| (pos.x, pos.y))
                .filter(|cell| want.contains(cell))
                .collect()
        };

        if existing.len() == wanted.len() && centre_glyph == Some(glyph) && stranded.is_empty() {
            return;
        }

        let have: HashSet<(i32, i32)> = existing.iter().map(|(_, pos, _)| *pos).collect();

        for &(entity, pos, _) in &existing {
            if !want.contains(&pos) {
                self.world.despawn(entity);
            }
        }

        let newly_covered: Vec<(i32, i32)> = wanted
            .into_iter()
            .filter(|cell| !have.contains(cell))
            .collect();

        let mut to_displace = newly_covered.clone();
        for cell in stranded {
            if !to_displace.contains(&cell) {
                to_displace.push(cell);
            }
        }
        self.displace(key, &to_displace);

        for cell in newly_covered {
            let mut spawned = self.world.spawn((
                Settlement { key },
                Position {
                    x: cell.0,
                    y: cell.1,
                },
                Glyph {
                    ch: glyph,
                    color: GlyphColor::Orange,
                },
            ));
            if cell == tile {
                spawned.insert(SettlementCentre);
            }
        }

        let mut repaint = self.world.query::<(&Settlement, &mut Glyph)>();
        for (settlement, mut drawn) in repaint.iter_mut(&mut self.world) {
            if settlement.key == key {
                drawn.ch = glyph;
            }
        }
    }

    /// Whether `(x, y)` is unclaimed ground a settlement search may land a
    /// party or a displaced occupant on — walkable, and none of wild
    /// creature, nest, surface link, settlement cell, trap or outpost.
    ///
    /// Shared by `free_tile_outside` and `relay_landing` (a call, not a
    /// copy) — widening the list once, as displacement did by adding traps
    /// and outposts, widens both rather than leaving a second copy to
    /// drift.
    fn settlement_search_tile_free(&mut self, x: i32, y: i32) -> bool {
        self.world
            .resource_mut::<crate::world::WorldMap>()
            .tile(x, y)
            .walkable
            && self.surface_wild_creature_at(x, y).is_none()
            && self.surface_caravan_at(x, y).is_none()
            && self.find_nest_at(x, y).is_none()
            && self.find_surface_link_at(x, y).is_none()
            && self.find_settlement_at(x, y).is_none()
            && self.find_trap_at(x, y).is_none()
            && !self.world.resource::<Outposts>().0.contains_key(&(x, y))
            && !self.player_or_anchor_at(x, y)
    }

    /// The player's `Position` is a surface tile in every locale — pinned to
    /// the anchor in base space, to the entrance in the Stack — so it is
    /// read without a locale check.
    fn player_or_anchor_at(&self, x: i32, y: i32) -> bool {
        let anchor = self.world.resource::<AnchorEntity>().0;
        [self.player_entity(), anchor].into_iter().any(|entity| {
            self.world
                .get::<Position>(entity)
                .is_some_and(|pos| pos.x == x && pos.y == y)
        })
    }

    /// `find_wild_creature_at` less a besieger, whose `Position` is a
    /// base-space cell a surface footprint can only collide with by number.
    fn surface_wild_creature_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        self.wild_creature_positions()
            .into_iter()
            .find(|(e, p)| p.x == x && p.y == y && !self.stands_in_base_space(*e))
            .map(|(e, _)| e)
    }

    /// A caravan inside base space is at a base-space cell, for
    /// `surface_wild_creature_at`'s reason.
    fn surface_caravan_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), With<Caravan>>();
        let hits: Vec<Entity> = query
            .iter(&self.world)
            .filter(|(_, pos)| pos.x == x && pos.y == y)
            .map(|(entity, _)| entity)
            .collect();
        hits.into_iter().find(|&e| !self.stands_in_base_space(e))
    }

    /// The nearest free tile outside `key`'s footprint — a displaced
    /// occupant's new home, and `relay_landing`'s search besides.
    ///
    /// Bands start at `radius + 1`, strictly outside the footprint's own
    /// final size, so this can never answer with a cell the footprint is
    /// about to cover even before every one of its cells is spawned — which
    /// is what lets `sync_settlement_footprint` call `displace` before it
    /// finishes spawning the cells displacement just cleared.
    pub(crate) fn free_tile_outside(&mut self, key: SettlementKey) -> Option<(i32, i32)> {
        let tile = self
            .world
            .resource::<crate::resources::Settlements>()
            .0
            .get(&key)?
            .tile;
        let radius = self.settlement_radius(key)?;
        let candidates = crate::game::spawning::ring_tiles(
            tile,
            radius + 1,
            crate::tuning::SETTLEMENT_SITE_SEARCH_TILES,
        );
        candidates
            .into_iter()
            .find(|&(x, y)| self.settlement_search_tile_free(x, y))
    }

    /// What happens to whatever already stood on a cell a footprint just
    /// grew over — the design's per-occupant table, run against the cells
    /// `sync_settlement_footprint` is about to spawn onto (plus any cell
    /// holding a Stack entrance stranded there by an earlier deferral).
    ///
    /// Draws no `resources::GameRng` — every mover below writes a
    /// `Position` (or, for a Stack entrance, calls `spawn_entrance_at`)
    /// with no roll of its own, `stack::generate`'s world-generation rule
    /// extended here: a growing town relocating what stood in its way is
    /// generation of the same kind, and a draw from the shared stream would
    /// shift every later roll in the run.
    fn displace(&mut self, key: SettlementKey, cells: &[(i32, i32)]) {
        for &(x, y) in cells {
            self.displace_wild_creature_at(key, x, y);
            self.displace_trap_at(key, x, y);
            self.displace_nest_at(key, x, y);
            self.displace_caravan_at(key, x, y);
            self.displace_player_at(key, x, y);
            self.displace_anchor_at(key, x, y);
            self.displace_stack_entrance_at(key, x, y);
        }
    }

    fn displace_wild_creature_at(&mut self, key: SettlementKey, x: i32, y: i32) {
        let Some(entity) = self.surface_wild_creature_at(x, y) else {
            return;
        };
        let Some((nx, ny)) = self.free_tile_outside(key) else {
            return;
        };
        if let Some(mut pos) = self.world.get_mut::<Position>(entity) {
            pos.x = nx;
            pos.y = ny;
        }
    }

    fn displace_trap_at(&mut self, key: SettlementKey, x: i32, y: i32) {
        let Some(entity) = self.find_trap_at(x, y) else {
            return;
        };
        let Some((nx, ny)) = self.free_tile_outside(key) else {
            return;
        };
        if let Some(mut pos) = self.world.get_mut::<Position>(entity) {
            pos.x = nx;
            pos.y = ny;
        }
    }

    /// Rewrites the nest's own `Position` and nothing else — a
    /// `NestGuardian` tethers by entity, so it follows without a second
    /// write.
    fn displace_nest_at(&mut self, key: SettlementKey, x: i32, y: i32) {
        let Some(entity) = self.find_nest_at(x, y) else {
            return;
        };
        let Some((nx, ny)) = self.free_tile_outside(key) else {
            return;
        };
        if let Some(mut pos) = self.world.get_mut::<Position>(entity) {
            pos.x = nx;
            pos.y = ny;
        }
    }

    /// `Position` and `arrival_tile` both move — leaving `arrival_tile`
    /// behind would send this caravan walking back into the settlement that
    /// just displaced it.
    fn displace_caravan_at(&mut self, key: SettlementKey, x: i32, y: i32) {
        let hit = self.surface_caravan_at(x, y);
        let Some(entity) = hit else {
            return;
        };
        let Some((nx, ny)) = self.free_tile_outside(key) else {
            return;
        };
        if let Some(mut pos) = self.world.get_mut::<Position>(entity) {
            pos.x = nx;
            pos.y = ny;
        }
        if let Some(mut caravan) = self.world.get_mut::<Caravan>(entity) {
            caravan.arrival_tile = (nx, ny);
        }
    }

    /// Only while `require_surface` holds — a `Position` that merely reads
    /// as this tile because it is pinned to a Stack entrance or a base
    /// anchor is not the player standing here, and moving it would fight
    /// the very pin that keeps it meaningful underground or in the base.
    fn displace_player_at(&mut self, key: SettlementKey, x: i32, y: i32) {
        if self.require_surface().is_err() {
            return;
        }
        let player = self.player_entity();
        let here = self
            .world
            .get::<Position>(player)
            .is_some_and(|pos| pos.x == x && pos.y == y);
        if !here {
            return;
        }
        let Some((nx, ny)) = self.free_tile_outside(key) else {
            return;
        };
        if let Some(mut pos) = self.world.get_mut::<Position>(player) {
            pos.x = nx;
            pos.y = ny;
        }
    }

    /// While the party is in base space the player's surface `Position` is
    /// pinned to the anchor tile, and `move_anchor_to` moves only the
    /// anchor, so the pin moves here — `leave_base` would otherwise step
    /// them back onto covered ground. Not inside `move_anchor_to`, whose
    /// founding caller runs before the party has ever entered base space.
    fn displace_anchor_at(&mut self, key: SettlementKey, x: i32, y: i32) {
        let anchor = self.world.resource::<AnchorEntity>().0;
        let here = self
            .world
            .get::<Position>(anchor)
            .is_some_and(|pos| pos.x == x && pos.y == y);
        if !here {
            return;
        }
        let Some((nx, ny)) = self.free_tile_outside(key) else {
            return;
        };
        if self.in_base() {
            let player = self.player_entity();
            if let Some(mut pos) = self.world.get_mut::<Position>(player)
                && pos.x == x
                && pos.y == y
            {
                pos.x = nx;
                pos.y = ny;
            }
        }
        self.move_anchor_to(nx, ny);
    }

    /// Relocated the way `collapse_stack` already does it — `erase_stack_
    /// entrance` despawns the link and drops that entrance's `StackMemory`,
    /// a fresh one opens at the new tile — except when the party is
    /// standing inside this very Stack (`Locale::Stack.entrance == (x,
    /// y)`), where relocating out from under them would fight `Position`'s
    /// own pin to the entrance while underground; that case is left for the
    /// next call after they surface, which `sync_settlement_footprint`'s
    /// stranded check exists to keep making.
    ///
    /// The destination is found **before** anything is erased, `collapse_
    /// stack`'s own order: nowhere to put it means nothing happens this
    /// tick rather than a Stack erased with no replacement.
    fn displace_stack_entrance_at(&mut self, key: SettlementKey, x: i32, y: i32) {
        if self.find_surface_link_at(x, y).is_none() {
            return;
        }
        if self.stack_pos().is_some_and(|pos| pos.entrance == (x, y)) {
            return;
        }
        let Some((nx, ny)) = self.free_tile_outside(key) else {
            return;
        };
        self.erase_stack_entrance((x, y));
        self.spawn_entrance_at(nx, ny);
        self.log_kind(
            MessageKind::Info,
            "A Stack entrance is swallowed as the settlement grows over it, \
             and opens again somewhere past its walls."
                .to_string(),
        );
    }
}
