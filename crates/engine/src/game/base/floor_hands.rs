//! The player's own hands on the floor: setting a thing down and picking it
//! up. Everything else that lands on the floor is a carrier's doing
//! (`floor.rs`).

use crate::Game;
use crate::components::{DownedPrograms, FloorItem, GearCopies, Inventory, Position};
use crate::game::base::floor::{floor_item_at, spawn_floor_item, take_floor_item};
use crate::items::FloorThing;
use crate::tuning::MAX_DOWNED_PROGRAMS;
use crate::views::{PutDownChoice, PutDownOption};

fn capitalised(s: String) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => s,
    }
}

impl Game {
    fn hands_free(&self) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Not right now.".into());
        }
        self.require_base()
    }

    fn party_cell(&self) -> Option<Position> {
        self.base_pos().map(|(x, y)| Position { x, y })
    }

    /// What the player could set down: carried items, then gear copies, then
    /// downed programs by store index. Banked currency is not carried, so it
    /// has no row.
    pub fn put_down_options(&self) -> Vec<PutDownOption> {
        let player = self.player_entity();
        let mut rows = Vec::new();
        if let Some(inv) = self.world.get::<Inventory>(player) {
            let db = self.world.resource::<crate::items_db::ItemDb>();
            for (id, qty) in &inv.items {
                if *qty == 0 || db.get(id.as_str()).is_some_and(|d| d.banked) {
                    continue;
                }
                rows.push(PutDownOption {
                    label: format!("{} x{qty}", self.item_name(id)),
                    choice: PutDownChoice::Item(id.clone()),
                });
            }
        }
        if let Some(copies) = self.world.get::<GearCopies>(player) {
            for (copy, qty) in &copies.copies {
                rows.push(PutDownOption {
                    label: format!("{} x{qty}", self.copy_name(copy)),
                    choice: PutDownChoice::Gear(copy.clone()),
                });
            }
        }
        if let Some(store) = self.world.get::<DownedPrograms>(player) {
            for (i, program) in store.0.iter().enumerate() {
                rows.push(PutDownOption {
                    label: capitalised(self.downed_program_label(program)),
                    choice: PutDownChoice::Program(i),
                });
            }
        }
        rows
    }

    /// Sets one unit of `choice` on the party's tile, or the nearest free
    /// one. Refused when there is none: the player's own drop never stows,
    /// since stowing would hand the thing straight back. Nothing moves
    /// before every guard has passed.
    pub fn put_down(&mut self, choice: &PutDownChoice) -> Result<(), String> {
        self.hands_free()?;
        let party = self
            .party_cell()
            .expect("hands_free passed, so the party is in base space");
        let cell = self
            .free_floor_cells(party, 1)
            .first()
            .copied()
            .ok_or_else(|| "There's no free floor in reach to put that down on.".to_string())?;
        let player = self.player_entity();
        let gone = || "That's no longer in your pack.".to_string();
        let (thing, name) = match choice {
            PutDownChoice::Item(id) => {
                let held = self
                    .world
                    .get::<Inventory>(player)
                    .is_some_and(|inv| inv.count(id) > 0);
                if !held {
                    return Err(gone());
                }
                let name = self.item_name(id).to_string();
                self.world
                    .get_mut::<Inventory>(player)
                    .expect("held above")
                    .take(id.clone(), 1);
                (FloorThing::Item(id.clone()), name)
            }
            PutDownChoice::Gear(copy) => {
                let held = self
                    .world
                    .get::<GearCopies>(player)
                    .is_some_and(|c| c.count(copy) > 0);
                if !held {
                    return Err(gone());
                }
                let name = self.copy_name(copy);
                self.world
                    .get_mut::<GearCopies>(player)
                    .expect("held above")
                    .take(copy, 1);
                (FloorThing::Gear(copy.clone()), name)
            }
            PutDownChoice::Program(i) => {
                let held = self
                    .world
                    .get::<DownedPrograms>(player)
                    .is_some_and(|s| *i < s.0.len());
                if !held {
                    return Err(gone());
                }
                let program = self
                    .world
                    .get_mut::<DownedPrograms>(player)
                    .expect("held above")
                    .0
                    .remove(*i);
                let name = self.downed_program_label(&program);
                (FloorThing::Program(program), name)
            }
        };
        spawn_floor_item(
            &mut self.world,
            Position {
                x: cell.0,
                y: cell.1,
            },
            thing,
        );
        self.log_base(format!("You put down {name}."));
        self.tick();
        Ok(())
    }

    /// Takes the thing on the party's tile back into its store.
    pub fn pick_up(&mut self) -> Result<(), String> {
        self.hands_free()?;
        let party = self
            .party_cell()
            .expect("hands_free passed, so the party is in base space");
        let e = floor_item_at(&mut self.world, party)
            .ok_or_else(|| "There's nothing here to pick up.".to_string())?;
        let player = self.player_entity();
        let program_store_full = matches!(
            self.world.get::<FloorItem>(e),
            Some(FloorItem(FloorThing::Program(_)))
        ) && self
            .world
            .get::<DownedPrograms>(player)
            .is_some_and(|s| s.0.len() >= MAX_DOWNED_PROGRAMS);
        if program_store_full {
            return Err("Your downed store is full — extract or rack one first.".into());
        }
        let thing =
            take_floor_item(&mut self.world, e).expect("floor_item_at returned a floor item");
        let name = match thing {
            FloorThing::Item(id) => {
                let name = self.item_name(&id).to_string();
                if let Some(mut inv) = self.world.get_mut::<Inventory>(player) {
                    inv.add(id, 1);
                }
                name
            }
            FloorThing::Gear(copy) => {
                let name = self.copy_name(&copy);
                if let Some(mut copies) = self.world.get_mut::<GearCopies>(player) {
                    copies.add(copy, 1);
                }
                name
            }
            FloorThing::Program(program) => {
                let name = self.downed_program_label(&program);
                if let Some(mut store) = self.world.get_mut::<DownedPrograms>(player) {
                    store.0.push(program);
                }
                name
            }
        };
        self.log_base(format!("You pick up {name}."));
        self.tick();
        Ok(())
    }
}
