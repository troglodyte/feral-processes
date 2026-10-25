//! What `[c]` can do with the structures beside the party: one entry per
//! structure, each with the pages the hub shows for it.

use crate::components::{BuildSite, FarmPlot, FarmTile};
use crate::tactical::footprint_cells_at;
use crate::*;

/// North, east, south, west — the order the hub cycles. Not
/// `collect::ORTHOGONAL`, which is `(x, y)`-scan order.
const CLOCKWISE: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

/// One page of the structure hub. Declaration order is the rail order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum StructurePage {
    Inventory,
    Filter,
    Recipe,
    Tool,
    Siphon,
    Splice,
    Grow,
    Rebuild,
    /// The Workbench's routine-crafting pages (ADR 0029), in rail order.
    Build,
    Subroutines,
    Built,
    /// The Workbench's recipe list: the same Compile screen `[b]` opens,
    /// with the bench-only recipes in it.
    Compile,
    Info,
}

/// A structure beside the party, in the hub's cycle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdjacentStructure {
    pub entity: Entity,
    /// The `(dx, dy)` step from the party to the first side (clockwise from
    /// north) this structure occupies.
    pub dir: (i32, i32),
    pub name: String,
    /// Rail order; `Info` is always last.
    pub pages: Vec<StructurePage>,
}

/// Something that could be in the cycle: its cells and the name shown.
struct Candidate {
    entity: Entity,
    cells: Vec<(i32, i32)>,
    name: String,
}

impl Game {
    /// Every structure with a cell orthogonally beside the party, once each,
    /// ordered by the first side it occupies going N, E, S, W. A structure
    /// with nothing but its `Info` page is not in the cycle.
    ///
    /// Empty outside base space, after game over and during a fight, the
    /// guards the transfer offers carry.
    pub fn adjacent_structures(&self) -> Vec<AdjacentStructure> {
        let Some((px, py)) = self.base_pos() else {
            return Vec::new();
        };
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Vec::new();
        }
        let mut found: Vec<(usize, (i32, i32), Entity, AdjacentStructure)> = Vec::new();
        for Candidate {
            entity,
            cells,
            name,
        } in self.hub_candidates()
        {
            let Some(side) = CLOCKWISE
                .iter()
                .position(|(dx, dy)| cells.contains(&(px + dx, py + dy)))
            else {
                continue;
            };
            let pages = self.structure_pages(entity);
            if pages == [StructurePage::Info] {
                continue;
            }
            let anchor = cells.iter().copied().min().unwrap_or((0, 0));
            found.push((
                side,
                anchor,
                entity,
                AdjacentStructure {
                    entity,
                    dir: CLOCKWISE[side],
                    name,
                    pages,
                },
            ));
        }
        found.sort_by_key(|(side, anchor, entity, _)| (*side, *anchor, *entity));
        found.into_iter().map(|(.., s)| s).collect()
    }

    /// The pages this structure offers, capability-driven, `Info` last.
    pub(crate) fn structure_pages(&self, e: Entity) -> Vec<StructurePage> {
        let mut pages = Vec::new();
        let mut page = |on: bool, p: StructurePage| {
            if on {
                pages.push(p);
            }
        };
        page(self.has_inventory_page(e), StructurePage::Inventory);
        page(self.is_depot(e), StructurePage::Filter);
        page(self.is_machine(e), StructurePage::Recipe);
        page(self.is_teardown_rig(e), StructurePage::Tool);
        page(self.is_siphon(e), StructurePage::Siphon);
        page(self.is_splice_rig(e), StructurePage::Splice);
        page(self.is_growable_plot(e), StructurePage::Grow);
        page(self.is_awaiting_site(e), StructurePage::Rebuild);
        let bench = self.is_routine_bench(e);
        page(bench, StructurePage::Build);
        page(bench, StructurePage::Subroutines);
        page(bench, StructurePage::Built);
        page(bench, StructurePage::Compile);
        pages.push(StructurePage::Info);
        pages
    }

    /// Whether the structure has cargo to take from or put to: a rack, an
    /// Index Terminal, or a shelf worth opening. A Teardown Rig's hopper is
    /// opened by its Tool page, so it earns Inventory only while its output
    /// buffer holds something.
    fn has_inventory_page(&self, e: Entity) -> bool {
        if self.is_rack(e) || self.indexes_storage(e) {
            return true;
        }
        let Some(stock) = self.world.get::<Stock>(e) else {
            return false;
        };
        let Some(def) = self.structure_def_of(e) else {
            return false;
        };
        let holds = stock.output.values().any(|n| *n > 0);
        if def.strips.is_some() {
            holds
        } else {
            def.stores || def.machine.is_some() || def.supply_capacity.is_some() || holds
        }
    }

    /// A farm plot, when there is a crop to pick for it.
    fn is_growable_plot(&self, e: Entity) -> bool {
        self.world.get::<FarmPlot>(e).is_some() && !self.crop_defs().is_empty()
    }

    fn is_awaiting_site(&self, e: Entity) -> bool {
        self.world
            .get::<BuildSite>(e)
            .is_some_and(|b| b.awaiting_program)
    }

    /// Everything that could be in the cycle with the cells it covers and
    /// the name the hub shows: standing structures, farm plots (their
    /// tiles) and rebuild sites awaiting a program.
    fn hub_candidates(&self) -> Vec<Candidate> {
        let mut out = Vec::new();
        for e in self.world.iter_entities() {
            let (Some(s), Some(p)) = (e.get::<Structure>(), e.get::<Position>()) else {
                continue;
            };
            let Some(def) = self.world.resource::<StructureDb>().get(&s.kind) else {
                continue;
            };
            out.push(Candidate {
                entity: e.id(),
                cells: footprint_cells_at((p.x, p.y), def.footprint),
                name: def.name.clone(),
            });
        }
        let mut plots: Vec<(Entity, Vec<(i32, i32)>)> = Vec::new();
        for e in self.world.iter_entities() {
            let (Some(tile), Some(p)) = (e.get::<FarmTile>(), e.get::<Position>()) else {
                continue;
            };
            match plots.iter_mut().find(|(plot, _)| *plot == tile.plot) {
                Some((_, cells)) => cells.push((p.x, p.y)),
                None => plots.push((tile.plot, vec![(p.x, p.y)])),
            }
        }
        out.extend(plots.into_iter().map(|(plot, cells)| Candidate {
            entity: plot,
            cells,
            name: "Farm plot".to_string(),
        }));
        for site in self.awaiting_program_sites() {
            let cells =
                footprint_cells_at((site.x, site.y), self.structure_footprint(&site.structure));
            let name = self
                .world
                .resource::<StructureDb>()
                .get(&site.structure)
                .map_or_else(
                    || site.structure.to_string(),
                    |d| format!("{} site", d.name),
                );
            out.push(Candidate {
                entity: site.site,
                cells,
                name,
            });
        }
        out
    }
}
