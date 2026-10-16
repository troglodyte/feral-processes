//! Farm plots: marked by box, one recipe per plot, one entity per cell.
//!
//! A `FarmTile` per cell because a crew `Task` targets one `Entity` (the
//! `DigSite` precedent); a `FarmPlot` per box because choosing a crop must be
//! one write. `DigSite` is not reused: its mark, durability and finish
//! semantics are a different verb.

use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::*;

use crate::Game;
use crate::base_grid::BaseGrid;
use crate::components::{CropGrowth, FarmPlot, FarmTile, Position};
use crate::crops::{CropDb, CropId};
use crate::views::{FarmTileState, FarmTileView};

/// Which of a crop's `stages` looks a tile at `elapsed` of `grow_ticks`
/// shows, clamped so the tick that ripens it never reads past the last look.
pub(crate) fn growth_stage(elapsed: u32, grow_ticks: u32, stages: u8) -> u8 {
    ((elapsed as u64 * stages as u64) / grow_ticks as u64).min(stages as u64 - 1) as u8
}

impl Game {
    /// Marks or clears a box of farm plot, `toggle_mark_box`'s rule: **the
    /// anchor `a` decides**, read before anything is written. Anchored on a
    /// plot tile it clears every plot tile in the box, whatever plot owns it;
    /// otherwise it marks one new plot from the box's eligible cells that are
    /// not already plot tiles, so an existing tile never changes plot.
    ///
    /// Eligible is laid floor with no structure or filed build on it. A plot
    /// left with no tiles is despawned, and a mark with no new cells spawns
    /// nothing.
    pub fn toggle_plot_box(&mut self, a: (i32, i32), b: (i32, i32)) {
        let tiles = self.plot_tiles_by_cell();
        let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
        let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
        let cells = (y0..=y1).flat_map(|y| (x0..=x1).map(move |x| (x, y)));
        if tiles.contains_key(&a) {
            let mut touched = HashSet::new();
            for cell in cells {
                if let Some(&tile) = tiles.get(&cell) {
                    if let Some(t) = self.world.get::<FarmTile>(tile) {
                        touched.insert(t.plot);
                    }
                    self.world.despawn(tile);
                }
            }
            for plot in touched {
                self.despawn_plot_if_empty(plot);
            }
            return;
        }
        let blocked: HashSet<(i32, i32)> = self
            .structure_tiles()
            .union(&self.build_site_tiles())
            .copied()
            .collect();
        let grid = self.world.resource::<BaseGrid>();
        let fresh: Vec<(i32, i32)> = cells
            .filter(|c| grid.is_floor(c.0, c.1) && !blocked.contains(c) && !tiles.contains_key(c))
            .collect();
        if fresh.is_empty() {
            return;
        }
        let plot = self.world.spawn(FarmPlot { recipe: None }).id();
        for (x, y) in fresh {
            self.world.spawn((
                Position { x, y },
                FarmTile {
                    plot,
                    growth: CropGrowth::Fallow,
                },
            ));
        }
    }

    fn despawn_plot_if_empty(&mut self, plot: Entity) {
        let mut query = self.world.query::<&FarmTile>();
        if !query.iter(&self.world).any(|t| t.plot == plot) {
            self.world.despawn(plot);
        }
    }

    /// Sows `plot` with `crop`. The same recipe again is a no-op that keeps
    /// growth; a different one leaves every tile fallow. It frees no worker
    /// itself — the crew step notices the tiles no longer match.
    pub fn set_plot_recipe(&mut self, plot: Entity, crop: &CropId) -> Result<(), String> {
        if self.world.get::<FarmPlot>(plot).is_none() {
            return Err("that is not a farm plot".to_string());
        }
        if self.world.resource::<CropDb>().get(crop).is_none() {
            return Err(format!("no crop called {}", crop.as_str()));
        }
        if self.plot_recipe(plot).as_ref() == Some(crop) {
            return Ok(());
        }
        self.world
            .get_mut::<FarmPlot>(plot)
            .expect("checked above")
            .recipe = Some(crop.clone());
        let mut query = self.world.query::<&mut FarmTile>();
        for mut tile in query.iter_mut(&mut self.world) {
            if tile.plot == plot {
                tile.growth = CropGrowth::Fallow;
            }
        }
        Ok(())
    }

    /// One tick of the farm clock: prune, then every tile of a sown plot
    /// advances. A tile ripens on the tick `elapsed` reaches `grow_ticks`;
    /// `Fallow`, `Ripe` and unsown tiles stand still, and a tick lost to a
    /// battle is lost — there is no catch-up.
    pub(crate) fn grow_farms(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        self.prune_farm_tiles();
        let mut plots = self.world.query::<(Entity, &FarmPlot)>();
        let db = self.world.resource::<CropDb>();
        let grow_ticks: HashMap<Entity, u32> = plots
            .iter(&self.world)
            .filter_map(|(plot, p)| {
                let def = db.get(p.recipe.as_ref()?)?;
                Some((plot, def.grow_ticks))
            })
            .collect();
        let mut tiles = self.world.query::<&mut FarmTile>();
        for mut tile in tiles.iter_mut(&mut self.world) {
            let (Some(&grow), CropGrowth::Growing { elapsed }) =
                (grow_ticks.get(&tile.plot), tile.growth)
            else {
                continue;
            };
            tile.growth = if elapsed + 1 >= grow {
                CropGrowth::Ripe
            } else {
                CropGrowth::Growing {
                    elapsed: elapsed + 1,
                }
            };
        }
    }

    /// Despawns every tile whose cell is no longer laid floor or now holds a
    /// structure or a filed build, and any plot that empties with them. One
    /// lazy pass instead of a hook in each writer — a build request, a hand
    /// deploy, `BaseGrid::revert` and a stale save all land here
    /// (`prune_finishes`' precedent).
    fn prune_farm_tiles(&mut self) {
        let blocked: HashSet<(i32, i32)> = self
            .structure_tiles()
            .union(&self.build_site_tiles())
            .copied()
            .collect();
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), With<FarmTile>>();
        let grid = self.world.resource::<BaseGrid>();
        let doomed: Vec<Entity> = query
            .iter(&self.world)
            .filter(|(_, p)| !grid.is_floor(p.x, p.y) || blocked.contains(&(p.x, p.y)))
            .map(|(e, _)| e)
            .collect();
        let mut touched = HashSet::new();
        for tile in doomed {
            if let Some(t) = self.world.get::<FarmTile>(tile) {
                touched.insert(t.plot);
            }
            self.world.despawn(tile);
        }
        for plot in touched {
            self.despawn_plot_if_empty(plot);
        }
    }

    pub fn plot_recipe(&self, plot: Entity) -> Option<CropId> {
        self.world.get::<FarmPlot>(plot)?.recipe.clone()
    }

    /// Every plot cell for the base map, in `pos` order. Empty outside base
    /// space for `floor_piles`' reason: a tile's `Position` is a base cell.
    pub fn farm_tiles(&self) -> Vec<FarmTileView> {
        if self.base_pos().is_none() {
            return Vec::new();
        }
        let Some(mut query) = self.world.try_query::<(&Position, &FarmTile)>() else {
            return Vec::new();
        };
        let db = self.world.resource::<CropDb>();
        let mut rows: Vec<FarmTileView> = query
            .iter(&self.world)
            .map(|(p, tile)| {
                let def = self
                    .world
                    .get::<FarmPlot>(tile.plot)
                    .and_then(|plot| plot.recipe.as_ref())
                    .and_then(|id| db.get(id));
                let state = match (tile.growth, def) {
                    (CropGrowth::Growing { elapsed }, Some(def)) => FarmTileState::Growing {
                        stage: growth_stage(elapsed, def.grow_ticks, def.stages),
                        stages: def.stages,
                    },
                    (CropGrowth::Ripe, Some(_)) => FarmTileState::Ripe,
                    _ => FarmTileState::Fallow,
                };
                FarmTileView {
                    pos: (p.x, p.y),
                    plot: tile.plot,
                    crop: def.map(|d| (d.glyph.clone(), d.color)),
                    state,
                }
            })
            .collect();
        rows.sort_unstable_by_key(|r| r.pos);
        rows
    }

    /// Every `FarmTile` in base space by its cell — the lookup a box walks
    /// against, built once per press as `dig_sites_by_tile` is.
    pub(crate) fn plot_tiles_by_cell(&mut self) -> HashMap<(i32, i32), Entity> {
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), With<FarmTile>>();
        query
            .iter(&self.world)
            .map(|(e, p)| ((p.x, p.y), e))
            .collect()
    }
}
