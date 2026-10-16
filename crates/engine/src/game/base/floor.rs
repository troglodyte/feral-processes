//! Loads on the floor: where a carrier's cargo goes when the carrier stops
//! holding it for any reason but delivery.
//!
//! `drop_load` is the only way a `Carrying` ends other than delivery
//! (`deposit` / `Load` at arrival). A later hauling errand brings the pile
//! home.

use std::collections::{BTreeMap, HashSet};

use bevy_ecs::prelude::*;

use crate::Game;
use crate::base_grid::BaseGrid;
use crate::components::{Carrying, FloorItem, FloorPile, Position};
use crate::game::base::hauling::Occupancy;
use crate::items::{FloorThing, ItemId};
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

/// The one place a floor item's component list is written. One item per
/// tile: callers pick a free cell through `Game::set_down`, and a second
/// writer onto an occupied tile would hide a thing from every
/// `floor_item_at` lookup.
pub(crate) fn spawn_floor_item(world: &mut World, pos: Position, thing: FloorThing) -> Entity {
    debug_assert!(
        floor_item_at(world, pos).is_none(),
        "a floor tile holds one item"
    );
    world.spawn((FloorItem(thing), pos)).id()
}

/// The item lying on `pos`, if any. A linear query: floor items are few.
pub(crate) fn floor_item_at(world: &mut World, pos: Position) -> Option<Entity> {
    world
        .query::<(Entity, &Position, &FloorItem)>()
        .iter(world)
        .find(|(_, p, _)| p.x == pos.x && p.y == pos.y)
        .map(|(e, _, _)| e)
}

/// Picks `e` up off the floor: despawns it and returns what it held.
#[expect(dead_code, reason = "Task 4 wires the pile paths onto floor items")]
pub(crate) fn take_floor_item(world: &mut World, e: Entity) -> Option<FloorThing> {
    let FloorItem(thing) = world.entity_mut(e).take::<FloorItem>()?;
    world.despawn(e);
    Some(thing)
}

/// Picks up `e` only if it is a plain `Item(item)`; gear and programs keep
/// their identity, so a unit-by-id request must not swallow them.
#[expect(dead_code, reason = "Task 4 wires the pile paths onto floor items")]
pub(crate) fn take_floor_unit(world: &mut World, e: Entity, item: &ItemId) -> bool {
    let matches = world
        .get::<FloorItem>(e)
        .is_some_and(|f| matches!(&f.0, FloorThing::Item(i) if i == item));
    if matches {
        world.despawn(e);
    }
    matches
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

/// The one predicate for "an item may be set down here", and for which cells
/// a drop's search may pass through. Bodies are left out on purpose: they
/// move, and a tile a body stands on is no less a place to leave something.
pub(crate) struct FreeFloor {
    taken: HashSet<(i32, i32)>,
    walls: Occupancy,
}

impl FreeFloor {
    /// Laid floor, walkable, and not spoken for by a structure, a build site
    /// or another floor item.
    fn free(&self, grid: &BaseGrid, cell: (i32, i32)) -> bool {
        grid.is_floor(cell.0, cell.1)
            && grid.walkable(cell.0, cell.1)
            && !self.walls.walls(cell)
            && !self.taken.contains(&cell)
    }

    /// Whether the search may walk on through `cell`: a structure's anchor
    /// is a wall, so nothing lands behind one that a body could not reach.
    fn crossable(&self, grid: &BaseGrid, cell: (i32, i32)) -> bool {
        grid.walkable(cell.0, cell.1) && !self.walls.walls(cell)
    }
}

impl Game {
    /// Snapshot of what blocks a set-down right now.
    pub(crate) fn free_floor(&mut self) -> FreeFloor {
        let mut taken = self.structure_tiles();
        taken.extend(self.build_site_tiles());
        taken.extend(
            self.world
                .query::<(&Position, &FloorItem)>()
                .iter(&self.world)
                .map(|(p, _)| (p.x, p.y)),
        );
        FreeFloor {
            taken,
            walls: self.blocked_tiles(),
        }
    }

    /// Up to `count` free floor cells around `from`, nearest first.
    pub(crate) fn free_floor_cells(&mut self, from: Position, count: usize) -> Vec<(i32, i32)> {
        let floor = self.free_floor();
        let grid = self.world.resource::<BaseGrid>();
        nearest_free_cells(
            (from.x, from.y),
            count,
            |c| floor.free(grid, c),
            |c| floor.crossable(grid, c),
        )
    }

    /// Lays `things` on the floor around `from`, one per tile, nearest first
    /// from a single search. What finds no tile goes to `stow`, so a set-down
    /// never destroys anything.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "Task 4 wires the pile paths onto floor items")
    )]
    pub(crate) fn set_down(&mut self, from: Position, things: Vec<FloorThing>) {
        let cells = self.free_floor_cells(from, things.len());
        let mut things = things.into_iter();
        for (cell, thing) in cells.into_iter().zip(things.by_ref()) {
            let pos = Position {
                x: cell.0,
                y: cell.1,
            };
            spawn_floor_item(&mut self.world, pos, thing);
        }
        for thing in things {
            self.stow(thing, from);
        }
    }

    /// The fallback for a thing with no floor to lie on, saying so in the
    /// log: pack for items and gear; for a program the downed store, then
    /// the nearest rack with room, then the store past its cap.
    fn stow(&mut self, thing: FloorThing, from: Position) {
        let player = self.player_entity();
        match thing {
            FloorThing::Item(id) => {
                let name = self.item_name(&id).to_string();
                self.log_base(format!(
                    "No free floor for {name} — it goes into your pack."
                ));
                if let Some(mut inv) = self.world.get_mut::<crate::components::Inventory>(player) {
                    inv.add(id, 1);
                }
            }
            FloorThing::Gear(copy) => {
                let name = self.copy_name(&copy);
                self.log_base(format!(
                    "No free floor for {name} — it goes into your pack."
                ));
                if let Some(mut copies) =
                    self.world.get_mut::<crate::components::GearCopies>(player)
                {
                    copies.add(copy, 1);
                }
            }
            FloorThing::Program(program) => self.stow_program(program, from),
        }
    }

    fn stow_program(&mut self, program: crate::items::DownedProgram, from: Position) {
        use crate::components::{DownedPrograms, Racked};
        let player = self.player_entity();
        let label = self.downed_program_label(&program);
        let under_cap = self
            .world
            .get::<DownedPrograms>(player)
            .is_some_and(|h| h.0.len() < crate::tuning::MAX_DOWNED_PROGRAMS);
        let rack = if under_cap {
            None
        } else {
            self.nearest_rack_with_room(from)
        };
        let ending = if under_cap {
            "it goes to your downed store."
        } else if rack.is_some() {
            "it goes onto the Quarantine Rack."
        } else {
            "it squeezes into your downed store past its limit."
        };
        self.log_base(format!("No free floor for {label} — {ending}"));
        if let Some(rack) = rack
            && let Some(mut shelf) = self.world.get_mut::<Racked>(rack)
        {
            shelf.0.push(program);
        } else if let Some(mut store) = self.world.get_mut::<DownedPrograms>(player) {
            store.0.push(program);
        }
    }

    /// The rack nearest `from` by `(chebyshev, x, y)` that has a free slot.
    fn nearest_rack_with_room(&mut self, from: Position) -> Option<Entity> {
        let mut racks: Vec<(i32, i32, i32, Entity)> = self
            .world
            .query::<(Entity, &Position, &crate::components::Racked)>()
            .iter(&self.world)
            .map(|(e, p, _)| ((p.x - from.x).abs().max((p.y - from.y).abs()), p.x, p.y, e))
            .collect();
        racks.sort();
        racks
            .into_iter()
            .map(|(.., e)| e)
            .find(|e| self.rack_room(*e) > 0)
    }

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

/// Up to `count` cells `free` admits, nearest first: `from` itself if free,
/// then breadth-first over `world::NEIGHBOURS`, searching only through cells
/// `crossable` admits (`from` is always expanded). One search that stops at
/// `count`; fewer come back once every reachable crossable cell has been
/// tried — the grid is finite, so the search always ends.
pub(crate) fn nearest_free_cells(
    from: (i32, i32),
    count: usize,
    free: impl Fn((i32, i32)) -> bool,
    crossable: impl Fn((i32, i32)) -> bool,
) -> Vec<(i32, i32)> {
    let mut found = Vec::new();
    if count == 0 {
        return found;
    }
    if free(from) {
        found.push(from);
    }
    let mut seen = std::collections::HashSet::from([from]);
    let mut queue = std::collections::VecDeque::from([from]);
    while found.len() < count {
        let Some(at) = queue.pop_front() else { break };
        for (dx, dy) in crate::world::NEIGHBOURS {
            let next = (at.0 + dx, at.1 + dy);
            if !seen.insert(next) {
                continue;
            }
            if free(next) {
                found.push(next);
                if found.len() == count {
                    return found;
                }
            }
            if crossable(next) {
                queue.push_back(next);
            }
        }
    }
    found
}
