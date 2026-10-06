//! Loads on the floor: where a carrier's cargo goes when the carrier stops
//! holding it for any reason but delivery.
//!
//! `drop_load` is the only way a `Carrying` ends other than delivery
//! (`deposit` / `Load` at arrival). A later hauling errand brings the pile
//! home.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;

use crate::Game;
use crate::components::{Carrying, FloorPile, Position};
use crate::items::ItemId;
use crate::items_db::ItemDb;
use crate::save::FloorPileSave;
use crate::views::FloorPileView;

/// The one place a pile's component list is written, `spawn_structure`'s
/// reason: a hand-written copy drifts silently. Merges into the pile already
/// on the tile, so there is at most one per tile. A zero `qty` spawns nothing.
pub(crate) fn spawn_floor_pile(world: &mut World, pos: Position, item: ItemId, qty: u32) {
    if qty == 0 {
        return;
    }
    if let Some(pile) = floor_pile_at(world, pos) {
        let mut pile = world.get_mut::<FloorPile>(pile).expect("pile just found");
        *pile.items.entry(item).or_insert(0) += qty;
        return;
    }
    world.spawn((
        FloorPile {
            items: BTreeMap::from([(item, qty)]),
        },
        pos,
    ));
}

/// The pile standing on `pos`, if any. A linear query: piles are few.
pub(crate) fn floor_pile_at(world: &mut World, pos: Position) -> Option<Entity> {
    world
        .query::<(Entity, &Position, &FloorPile)>()
        .iter(world)
        .find(|(_, p, _)| p.x == pos.x && p.y == pos.y)
        .map(|(e, _, _)| e)
}

/// Puts `who`'s load on the floor of its tile and ends the hold. A no-op if
/// `who` carries nothing or has no tile.
pub(crate) fn drop_load(world: &mut World, who: Entity) {
    let Some(pos) = world.get::<Position>(who).copied() else {
        return;
    };
    let Some(load) = world.entity_mut(who).take::<Carrying>() else {
        return;
    };
    spawn_floor_pile(world, pos, load.item, load.qty);
}

/// Takes up to `qty` of `item` from `pile`, despawning it once empty.
/// Returns what was taken; 0 for a vanished pile or an item it lacks.
pub(crate) fn take_from_pile(world: &mut World, pile: Entity, item: &ItemId, qty: u32) -> u32 {
    let Some(mut contents) = world.get_mut::<FloorPile>(pile) else {
        return 0;
    };
    let Some(held) = contents.items.get_mut(item) else {
        return 0;
    };
    let taken = qty.min(*held);
    *held -= taken;
    if *held == 0 {
        contents.items.remove(item);
    }
    if contents.items.is_empty() {
        world.despawn(pile);
    }
    taken
}

impl Game {
    /// Every pile on the base floor, for the map and the examine line. Empty
    /// outside base space: a pile's `Position` is a base cell, and the surface
    /// map would draw it on whatever tile shares the coordinates.
    pub fn floor_piles(&self) -> Vec<FloorPileView> {
        if self.base_pos().is_none() {
            return Vec::new();
        }
        let Some(mut query) = self.world.try_query::<(&Position, &FloorPile)>() else {
            return Vec::new();
        };
        let db = self.world.resource::<ItemDb>();
        let mut rows: Vec<FloorPileView> = query
            .iter(&self.world)
            .map(|(p, pile)| FloorPileView {
                pos: (p.x, p.y),
                items: pile
                    .items
                    .iter()
                    .map(|(id, qty)| {
                        let name = db
                            .get(&id.0)
                            .map_or_else(|| id.to_string(), |d| d.name.clone());
                        (name, *qty)
                    })
                    .collect(),
            })
            .collect();
        rows.sort_unstable_by_key(|r| r.pos);
        rows
    }

    /// "a pile on the floor: 3 scrap, 1 wire" for the pile on `(x, y)`.
    pub(crate) fn describe_floor_pile(&self, x: i32, y: i32) -> Option<String> {
        let row = self.floor_piles().into_iter().find(|r| r.pos == (x, y))?;
        let held: Vec<String> = row
            .items
            .iter()
            .map(|(name, qty)| format!("{qty} {name}"))
            .collect();
        Some(format!("a pile on the floor: {}", held.join(", ")))
    }

    pub(crate) fn floor_pile_saves(&mut self) -> Vec<FloorPileSave> {
        self.world
            .query::<(&Position, &FloorPile)>()
            .iter(&self.world)
            .map(|(p, pile)| FloorPileSave {
                position: (p.x, p.y),
                items: pile.items.iter().map(|(i, n)| (i.clone(), *n)).collect(),
            })
            .collect()
    }

    pub(crate) fn restore_floor_piles(&mut self, piles: Vec<FloorPileSave>) {
        for saved in piles {
            let pos = Position {
                x: saved.position.0,
                y: saved.position.1,
            };
            for (item, qty) in saved.items {
                spawn_floor_pile(&mut self.world, pos, item, qty);
            }
        }
    }
}
