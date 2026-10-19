//! Setting what a configurable machine makes.
//!
//! A change is the one moment a machine's buffers and its posted worker stop
//! matching its recipe, so `set_machine_recipe` empties both before it writes
//! the new recipe: nothing a machine held is destroyed, and the scheduler
//! re-posts a worker against the new pace on its own.

use bevy_ecs::prelude::*;

use crate::Game;
use crate::components::{
    MachineRecipe, Position, ResourceNode, Stock, Structure, StructureTier, Task, TaskKind,
};
use crate::game::base::stock;
use crate::items::{FloorThing, ItemId};
use crate::items_db::ItemDb;
use crate::research::ResearchDb;
use crate::structures::{MachineRole, Placed, RecipeDef, StructureDb};
use crate::views::{MachineRecipeRow, MachineRecipeView, RecipePace};

impl Game {
    /// A standing structure as placed: its def and the recipe this instance
    /// is set to.
    pub(crate) fn placed(&self, structure: Entity) -> Option<Placed<'_>> {
        let kind = &self.world.get::<Structure>(structure)?.kind;
        let def = self.world.resource::<StructureDb>().get(kind)?;
        let recipe = self
            .world
            .get::<MachineRecipe>(structure)
            .and_then(|r| r.0.as_ref());
        Some(Placed { def, recipe })
    }

    /// Whether this structure is a configurable machine — the per-entity test
    /// the structure hub's Recipe page asks.
    pub(crate) fn is_machine(&self, structure: Entity) -> bool {
        self.structure_def_of(structure)
            .is_some_and(|d| d.machine.is_some())
    }

    /// What the recipe menu draws for `machine`; `None` when it is not a
    /// configurable machine, so a demolished one closes the screen.
    pub fn machine_recipe_view(&self, machine: Entity) -> Option<MachineRecipeView> {
        let placed = self.placed(machine)?;
        let def = placed.def.machine.as_ref()?;
        let &Position { x, y } = self.world.get::<Position>(machine)?;
        let items = self.world.resource::<ItemDb>();
        let rows = def
            .recipes
            .iter()
            .filter(|r| r.research.as_ref().is_none_or(|id| self.is_researched(id)))
            .map(|r| {
                let inputs = match def.role {
                    MachineRole::Extract => Vec::new(),
                    MachineRole::Assemble => items
                        .get(r.produces.as_str())
                        .and_then(|d| d.craftable.as_ref())
                        .map(|c| {
                            c.cost
                                .iter()
                                .map(|(item, qty)| (self.item_name(item).to_string(), *qty))
                                .collect()
                        })
                        .unwrap_or_default(),
                };
                MachineRecipeRow {
                    item: r.produces.clone(),
                    name: self.item_name(&r.produces).to_string(),
                    inputs,
                    pace: RecipePace::of(r.ticks_per_unit),
                    current: placed.recipe == Some(&r.produces),
                }
            })
            .collect();
        let held = self.world.get::<Stock>(machine).map_or(0, |s| {
            s.input.values().sum::<u32>() + s.output.values().sum::<u32>()
        });
        Some(MachineRecipeView {
            tile: (x, y),
            name: placed.def.name.clone(),
            current: placed.recipe.cloned(),
            held,
            rows,
        })
    }

    /// Sets `machine` to make `item`, emptying what it held and unposting its
    /// worker. Refusals, in order, all before anything changes: game over or
    /// a battle, away from the base, not a machine, a recipe the machine does
    /// not list, one not yet researched, the one it already makes.
    pub fn set_machine_recipe(&mut self, machine: Entity, item: &ItemId) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".to_string());
        }
        self.require_base()?;
        let placed = self
            .placed(machine)
            .filter(|p| p.def.machine.is_some())
            .ok_or_else(|| "That structure has nothing to set.".to_string())?;
        let kind = placed.def.name.clone();
        let item_name = self.item_name(item).to_string();
        let machine_def = placed.def.machine.as_ref().expect("filtered above");
        let role = machine_def.role;
        let recipe = machine_def
            .recipes
            .iter()
            .find(|r| &r.produces == item)
            .ok_or_else(|| format!("A {kind} can't make {item_name}."))?
            .clone();
        if let Some(gate) = recipe
            .research
            .as_ref()
            .filter(|id| !self.is_researched(id))
        {
            let name = self
                .world
                .resource::<ResearchDb>()
                .get(gate)
                .map_or(gate.as_str(), |d| d.name.as_str());
            return Err(format!("{item_name} needs {name} researched first."));
        }
        if placed.recipe == Some(item) {
            return Err(format!("It is already making {item_name}."));
        }

        let (to_depots, to_floor) = self.empty_machine(machine);
        self.unpost_worker(machine);

        self.write_machine_recipe(machine, role, &recipe);

        let mut line = format!("The {kind} is set to make {item_name}.");
        if to_depots > 0 {
            line.push_str(" What it held goes to the Depots.");
        }
        if to_floor > 0 {
            line.push_str(" What it held is left on the floor beside it.");
        }
        self.log_base(line);
        Ok(())
    }

    /// Writes `recipe` onto `machine`: the `MachineRecipe` and, for the
    /// Extract role, the `ResourceNode` at the machine's tier. The one write
    /// path `set_machine_recipe` and a rebuilt wreck share.
    fn write_machine_recipe(&mut self, machine: Entity, role: MachineRole, recipe: &RecipeDef) {
        self.world
            .entity_mut(machine)
            .insert(MachineRecipe(Some(recipe.produces.clone())));
        match role {
            MachineRole::Extract => {
                let tier = self.world.get::<StructureTier>(machine).map_or(1, |t| t.0);
                self.world.entity_mut(machine).insert(ResourceNode {
                    resource: recipe.produces.clone(),
                    level: recipe.level.map(|_| tier),
                });
            }
            MachineRole::Assemble => {
                self.world.entity_mut(machine).remove::<ResourceNode>();
            }
        }
    }

    /// Sets a freshly raised machine to the recipe its wreck made, if its
    /// def still lists it. Restores the player's choice; nothing here picks
    /// one, so it is not an auto-setup.
    pub(crate) fn restore_machine_recipe(&mut self, machine: Entity, item: &ItemId) {
        let Some((role, recipe)) = self.placed(machine).and_then(|p| {
            let machine_def = p.def.machine.as_ref()?;
            Some((machine_def.role, p.def.machine_recipe(item)?.clone()))
        }) else {
            return;
        };
        self.write_machine_recipe(machine, role, &recipe);
    }

    /// Empties both buffers: into the Depots as far as they have room, the
    /// rest set down on the floor around the machine. Returns the units that
    /// went to each.
    fn empty_machine(&mut self, machine: Entity) -> (u32, u32) {
        let Some(mut stock) = self.world.get_mut::<Stock>(machine) else {
            return (0, 0);
        };
        let held: Vec<(ItemId, u32)> = std::mem::take(&mut stock.input)
            .into_iter()
            .chain(std::mem::take(&mut stock.output))
            .filter(|(_, qty)| *qty > 0)
            .collect();
        let pos = *self
            .world
            .get::<Position>(machine)
            .expect("a machine stands on a tile");
        let (mut to_depots, mut to_floor) = (0, 0);
        let mut things = Vec::new();
        for (item, qty) in held {
            let landed = stock::return_to_depots(self, &item, qty);
            to_depots += landed;
            to_floor += qty - landed;
            things.extend((landed..qty).map(|_| FloorThing::Item(item.clone())));
        }
        self.set_down(pos, things);
        (to_depots, to_floor)
    }

    /// Takes every body working `machine` off it, loads on the floor: a
    /// posted program and a hand-worker can both hold a task here, and each
    /// has the old pace baked in. The scheduler re-posts the program at the
    /// new pace.
    fn unpost_worker(&mut self, machine: Entity) {
        let workers: Vec<Entity> = self
            .world
            .query::<(Entity, &Task)>()
            .iter(&self.world)
            .filter(|(_, t)| t.target == machine && matches!(t.kind, TaskKind::GatherResource))
            .map(|(e, _)| e)
            .collect();
        for worker in workers {
            self.drop_load(worker);
            self.world.entity_mut(worker).remove::<Task>();
        }
    }
}
