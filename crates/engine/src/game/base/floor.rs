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
use crate::save::FloorPileSave;

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
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "wired to its callers in the next commit")
)]
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
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "wired to the Pickup errand in a later commit")
)]
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
