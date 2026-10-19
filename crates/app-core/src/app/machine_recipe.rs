//! The recipe menu: what the Extractor, Factory, Assembler or Printer
//! beside the party makes.
//!
//! A page of the structure hub (`app/structure_hub.rs`) on a machine, re-read
//! on every key; the hub closes it the moment the machine stops being one
//! the player is standing at.
//!
//! Picking a row on an unset machine sets it at once. Picking one on a set
//! machine asks first (`Mode::MachineRecipeConfirm`), because a change
//! empties what the machine held.

use crate::*;

/// Which machine the screen is setting and what it last read of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineRecipeScreen {
    pub machine: Entity,
    pub view: MachineRecipeView,
    /// The row awaiting a yes or no while in `Mode::MachineRecipeConfirm`.
    pub pending: Option<ItemId>,
}

impl MachineRecipeScreen {
    /// The question the confirm page asks, naming what the change empties.
    /// `None` when nothing is pending.
    pub fn confirm_prompt(&self) -> Option<String> {
        let pending = self.pending.as_ref()?;
        let item = self
            .view
            .rows
            .iter()
            .find(|r| &r.item == pending)
            .map_or_else(|| pending.to_string(), |r| r.name.clone());
        let mut prompt = format!("Switch the {} to {}?", self.view.name, item);
        if self.view.held > 0 {
            prompt.push_str(&format!(
                " It empties {} units to the Depots or the floor beside it.",
                self.view.held
            ));
        }
        Some(prompt)
    }
}

impl App {
    /// Opens the recipe menu on one adjacent machine, the hub's Recipe page.
    pub(crate) fn open_machine_recipe_for(&mut self, machine: Entity) {
        if self.show_machine_recipe(machine) {
            self.menu_selected = 0;
            self.mode = Mode::MachineRecipe;
        }
    }

    /// Re-reads one machine into the screen, reporting whether it is still
    /// there. `false` leaves the screen empty; the hub's refresh closes it.
    fn show_machine_recipe(&mut self, machine: Entity) -> bool {
        let view = self
            .game
            .as_ref()
            .and_then(|g| g.machine_recipe_view(machine));
        let Some(view) = view else {
            self.machine_recipe = None;
            return false;
        };
        let pending = self.machine_recipe.take().and_then(|s| s.pending);
        self.machine_recipe = Some(MachineRecipeScreen {
            machine,
            view,
            pending,
        });
        true
    }

    /// Re-reads the open machine, answering it while the screen is still
    /// good.
    fn revalidate_machine_recipe(&mut self) -> Option<Entity> {
        let machine = self.machine_recipe.as_ref()?.machine;
        self.show_machine_recipe(machine).then_some(machine)
    }

    pub(crate) fn handle_machine_recipe_key(&mut self, key: GameKey) {
        let Some(machine) = self.revalidate_machine_recipe() else {
            return;
        };
        let rows = self
            .machine_recipe
            .as_ref()
            .map_or(0, |s| s.view.rows.len());
        match key {
            GameKey::Up | GameKey::Down => self.scroll(key, rows),
            GameKey::Enter => self.pick_machine_recipe(machine),
            key => {
                if let Some(idx) = self.selected_index(key, rows) {
                    self.menu_selected = idx;
                    self.pick_machine_recipe(machine);
                }
            }
        }
    }

    fn pick_machine_recipe(&mut self, machine: Entity) {
        let Some(screen) = &self.machine_recipe else {
            return;
        };
        let Some(row) = screen.view.rows.get(self.menu_selected) else {
            return;
        };
        let item = row.item.clone();
        if screen.view.current.is_none() {
            self.apply_machine_recipe(machine, &item);
        } else {
            if let Some(screen) = &mut self.machine_recipe {
                screen.pending = Some(item);
            }
            // The confirm's two rows are picked from the top, and a cursor
            // left on a deep recipe row would sit past them.
            self.menu_selected = 0;
            self.mode = Mode::MachineRecipeConfirm;
        }
    }

    fn apply_machine_recipe(&mut self, machine: Entity, item: &ItemId) {
        let Some(game) = &mut self.game else {
            return;
        };
        let outcome = game.set_machine_recipe(machine, item);
        self.report(outcome);
        if let Some(screen) = &mut self.machine_recipe {
            screen.pending = None;
        }
        self.mode = Mode::MachineRecipe;
        self.revalidate_machine_recipe();
        self.after_tick();
    }

    pub(crate) fn handle_machine_recipe_confirm_key(&mut self, key: GameKey) {
        let Some(machine) = self.revalidate_machine_recipe() else {
            return;
        };
        let pending = self.machine_recipe.as_ref().and_then(|s| s.pending.clone());
        let Some(item) = pending else {
            self.mode = Mode::MachineRecipe;
            return;
        };
        if key == GameKey::Esc {
            self.decline_machine_recipe();
            return;
        }
        match self.yes_no(key) {
            Some(true) => self.apply_machine_recipe(machine, &item),
            Some(false) => self.decline_machine_recipe(),
            None => {}
        }
    }

    fn decline_machine_recipe(&mut self) {
        if let Some(screen) = &mut self.machine_recipe {
            let asked = screen.pending.take();
            self.menu_selected = asked
                .and_then(|item| screen.view.rows.iter().position(|r| r.item == item))
                .unwrap_or(0);
        }
        self.mode = Mode::MachineRecipe;
    }
}
