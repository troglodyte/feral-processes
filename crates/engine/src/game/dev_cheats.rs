//! The dev keypad's engine half. `#[doc(hidden)] pub fn dev_*`, no `cfg`,
//! gated only in app-core — `dev_force_raid`'s convention. Each cheat calls
//! the real game code rather than writing state, so what is tested after
//! cheating is what a player gets.

use crate::items::ItemId;
use crate::*;

impl Game {
    /// Adds `qty` of a plain item to the player's pack. Gear is refused: it
    /// lives in `GearCopies` with rolled affixes, not in `Inventory`.
    #[doc(hidden)]
    pub fn dev_give_item(&mut self, id: &ItemId, qty: u32) -> Result<String, String> {
        let Some(def) = self.item_def(id) else {
            return Err(format!("No item called {id}."));
        };
        if def.equipment.is_some() {
            return Err(format!(
                "{} is gear; the cheat gives plain items only.",
                def.name
            ));
        }
        let player = self.player_entity();
        self.world
            .get_mut::<Inventory>(player)
            .ok_or("The player has no pack.")?
            .add(id.clone(), qty);
        Ok(format!("{qty} × {}", def.name))
    }

    /// Every plain item as (id, display name, held count), sorted by name,
    /// for the picker — a view, so a frontend never filters `ItemDb`.
    #[doc(hidden)]
    pub fn dev_plain_items(&self) -> Vec<(ItemId, String, u32)> {
        let inv = self.world.get::<Inventory>(self.player_entity());
        let mut rows: Vec<(ItemId, String, u32)> = self
            .world
            .resource::<ItemDb>()
            .all()
            .filter(|def| def.equipment.is_none())
            .map(|def| {
                let held = inv.map_or(0, |i| i.count(&def.id));
                (def.id.clone(), def.name.clone(), held)
            })
            .collect();
        rows.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        rows
    }
}
