//! Which machines have an ingredient that nothing in the base supplies.
//!
//! Base-wide where `lines::missing_ingredient` is line-wide: a hauler walks
//! a far extractor's output over, so an adjacent feeder is not the test. A
//! source is another structure that makes the item, a farm plot set to grow
//! it, a Depot shelf holding a batch, or a batch already in the machine's
//! own input. Read off how the base is built rather than `Starved`, which
//! also fires while a supplier that exists is merely slow.

use std::collections::{BTreeMap, HashSet};

use bevy_ecs::prelude::*;

use crate::Game;
use crate::components::{FarmPlot, Stock, Structure};
use crate::crops::CropDb;
use crate::game::base::work_orders::{batch_within_reach, depot_holding};
use crate::items::ItemId;
use crate::items_db::ItemDb;

/// Whether some ingredient of `recipe` has no source: nothing in `sourced`
/// makes it, and `held` plus `in_store` is short of one batch.
pub(crate) fn lacks_a_source(
    recipe: &[(ItemId, u32)],
    held: &BTreeMap<ItemId, u32>,
    sourced: &HashSet<ItemId>,
    in_store: impl Fn(&ItemId) -> u32,
) -> bool {
    recipe.iter().any(|(item, per_batch)| {
        !sourced.contains(item)
            && !batch_within_reach(
                held.get(item).copied().unwrap_or(0),
                0,
                in_store(item),
                *per_batch,
            )
    })
}

impl Game {
    /// Every assembler with an ingredient `lacks_a_source` finds no source
    /// for. A machine's own product does not count as its own source.
    pub(crate) fn unsupplied_machines(&mut self) -> HashSet<Entity> {
        let structures: Vec<Entity> = self
            .world
            .query_filtered::<Entity, With<Structure>>()
            .iter(&self.world)
            .collect();
        let sown: Vec<_> = self
            .world
            .query::<&FarmPlot>()
            .iter(&self.world)
            .filter_map(|p| p.recipe.clone())
            .collect();
        let crops = self.world.resource::<CropDb>();
        let grown: HashSet<ItemId> = sown
            .iter()
            .filter_map(|id| crops.get(id))
            .map(|c| c.produces.clone())
            .collect();
        let makers: Vec<(Entity, ItemId)> = structures
            .iter()
            .filter_map(|&e| Some((e, crate::systems::produced_item(self.placed(e)?)?.clone())))
            .collect();
        let items = self.world.resource::<ItemDb>();
        structures
            .into_iter()
            .filter(|&machine| {
                let Some(recipe) = self
                    .placed(machine)
                    .and_then(|p| crate::systems::assembly_recipe(p, items))
                else {
                    return false;
                };
                let mut sourced = grown.clone();
                sourced.extend(
                    makers
                        .iter()
                        .filter(|(e, _)| *e != machine)
                        .map(|(_, item)| item.clone()),
                );
                let empty = BTreeMap::new();
                let held = self
                    .world
                    .get::<Stock>(machine)
                    .map_or(&empty, |s| &s.input);
                lacks_a_source(recipe, held, &sourced, |item| depot_holding(self, item))
            })
            .collect()
    }
}
