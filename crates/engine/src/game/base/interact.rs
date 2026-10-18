//! What the interact key can do with the structures beside the party.

use crate::game::base::collect::ORTHOGONAL;
use crate::*;

/// One thing `[c]` can do, and which neighbouring tile it is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Interaction {
    /// The `(dx, dy)` step from the party to the structure.
    pub dir: (i32, i32),
    pub kind: InteractionKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionKind {
    /// The transfer screen, which reaches every shelf and rack beside the
    /// party at once — so it is one interaction, not one per Depot.
    Transfer,
    /// Commit a program to the rebuild site (the entity) on that tile.
    RebuildProgram(Entity),
    /// Pick what the farm plot (the entity) grows.
    Plot(Entity),
    /// Set what the machine (the entity) makes — the recipe menu.
    Machine(Entity),
}

impl Game {
    /// Everything `[c]` could do from where the party stands, in
    /// `(dx, dy)` order of `ORTHOGONAL` per kind: the transfer first, then
    /// each awaiting rebuild site on a neighbouring tile, then each farm
    /// plot, then each machine with a recipe to set.
    ///
    /// Empty outside base space and during a fight, the same guards the
    /// transfer offers carry.
    pub fn adjacent_interactions(&self) -> Vec<Interaction> {
        let Some((px, py)) = self.base_pos() else {
            return Vec::new();
        };
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Vec::new();
        }
        let mut found = Vec::new();
        let transferable = !self.transfer_offer().is_empty()
            || !self.rack_offer().is_empty()
            || !self.adjacent_depot_entities().is_empty();
        if transferable {
            found.push(Interaction {
                dir: self.transfer_direction(px, py).unwrap_or((0, 0)),
                kind: InteractionKind::Transfer,
            });
        }
        for site in self.awaiting_program_sites() {
            let cells = crate::tactical::footprint_cells_at(
                (site.x, site.y),
                self.structure_footprint(&site.structure),
            );
            if let Some(&dir) = ORTHOGONAL
                .iter()
                .find(|(dx, dy)| cells.contains(&(px + dx, py + dy)))
            {
                found.push(Interaction {
                    dir,
                    kind: InteractionKind::RebuildProgram(site.site),
                });
            }
        }
        found.extend(self.plot_interactions(px, py));
        found.extend(self.machine_interactions(px, py));
        found
    }

    /// One interaction per machine beside the party, in `adjacent_machines`'
    /// own `(x, y)` order, aimed at the side its anchor is on.
    fn machine_interactions(&self, px: i32, py: i32) -> Vec<Interaction> {
        self.adjacent_machines()
            .into_iter()
            .filter_map(|machine| {
                let p = self.world.get::<Position>(machine)?;
                Some(Interaction {
                    dir: (p.x - px, p.y - py),
                    kind: InteractionKind::Machine(machine),
                })
            })
            .collect()
    }

    /// One interaction per distinct farm plot with a tile orthogonally
    /// beside the party. Standing on a tile alone offers nothing: `[c]`
    /// aims at a neighbour. None when no crop is loaded, since a plot with
    /// nothing to grow has no picker to open.
    fn plot_interactions(&self, px: i32, py: i32) -> Vec<Interaction> {
        if self.crop_defs().is_empty() {
            return Vec::new();
        }
        let tiles = self.farm_tiles();
        let mut out: Vec<Interaction> = Vec::new();
        for (dx, dy) in ORTHOGONAL {
            for t in tiles.iter().filter(|t| t.pos == (px + dx, py + dy)) {
                let kind = InteractionKind::Plot(t.plot);
                if out.iter().all(|i| i.kind != kind) {
                    out.push(Interaction {
                        dir: (dx, dy),
                        kind,
                    });
                }
            }
        }
        out
    }

    /// The side the transfer's first shelf or rack is on, falling back to
    /// the neighbouring storage-indexing structure (an Index Terminal holds
    /// no stock itself), lowest `(x, y)` first so the answer does not depend
    /// on spawn order. A plain shelf (a Depot) comes before a machine's own
    /// shelf whatever their order, because a machine's side is already
    /// claimed by its recipe offer and the arrow toward a visible Depot
    /// must open the cargo. `None` when the offer is the pack alone — the
    /// interaction then sits at `(0, 0)`, which no key can aim at.
    fn transfer_direction(&self, px: i32, py: i32) -> Option<(i32, i32)> {
        let side_of = |e: Entity| {
            let p = self.world.get::<Position>(e)?;
            ORTHOGONAL
                .iter()
                .copied()
                .find(|(dx, dy)| (p.x, p.y) == (px + dx, py + dy))
        };
        let db = self.world.resource::<StructureDb>();
        let is_machine = |e: &Entity| {
            self.world
                .get::<Structure>(*e)
                .and_then(|s| db.get(&s.kind))
                .is_some_and(|d| d.machine.is_some())
        };
        let (machines, shelves): (Vec<Entity>, Vec<Entity>) =
            self.adjacent_stock().into_iter().partition(is_machine);
        shelves
            .into_iter()
            .chain(self.adjacent_racks())
            .chain(machines)
            .find_map(side_of)
            .or_else(|| {
                let db = self.world.resource::<StructureDb>();
                let mut terminals: Vec<(i32, i32, Entity)> = self
                    .world
                    .iter_entities()
                    .filter(|e| {
                        e.get::<Structure>()
                            .and_then(|s| db.get(&s.kind))
                            .is_some_and(|d| d.indexes_storage)
                    })
                    .filter_map(|e| {
                        let p = e.get::<Position>()?;
                        Some((p.x, p.y, e.id()))
                    })
                    .collect();
                terminals.sort();
                terminals.into_iter().find_map(|(_, _, e)| side_of(e))
            })
    }
}
