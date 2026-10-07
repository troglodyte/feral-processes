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
}

impl Game {
    /// Everything `[c]` could do from where the party stands, in
    /// `(dx, dy)` order of `ORTHOGONAL` per kind: the transfer first, then
    /// each awaiting rebuild site on a neighbouring tile.
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
        if transferable && let Some(dir) = self.transfer_direction(px, py) {
            found.push(Interaction {
                dir,
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
        found
    }

    /// The side the transfer's first shelf or rack is on, falling back to
    /// the first neighbouring structure for an Index Terminal that holds
    /// no stock itself. `None` when the offer is the pack alone.
    fn transfer_direction(&self, px: i32, py: i32) -> Option<(i32, i32)> {
        let side_of = |e: Entity| {
            let p = self.world.get::<Position>(e)?;
            ORTHOGONAL
                .iter()
                .copied()
                .find(|(dx, dy)| (p.x, p.y) == (px + dx, py + dy))
        };
        self.adjacent_stock()
            .into_iter()
            .chain(self.adjacent_racks())
            .find_map(side_of)
            .or_else(|| {
                self.world
                    .iter_entities()
                    .filter(|e| e.contains::<Structure>())
                    .find_map(|e| side_of(e.id()))
            })
    }
}
