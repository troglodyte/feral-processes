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

    /// The zones a warp may target: the next `DEV_WARP_SPAN` after the
    /// current one. Bounded because each warped zone runs a full
    /// `enter_next_zone`; the game has no last zone to cap at.
    #[doc(hidden)]
    pub fn dev_warp_targets(&self) -> Vec<u32> {
        let current = self.world.resource::<ZoneLevel>().0;
        (current + 1..=current + crate::tuning::DEV_WARP_SPAN).collect()
    }

    /// `warp_to_zone` behind the keypad's refusals: never mid-battle.
    #[doc(hidden)]
    pub fn dev_warp_to_zone(&mut self, zone: u32) -> Result<String, String> {
        if self.has_active_battle() {
            return Err("Can't warp during a battle.".into());
        }
        self.warp_to_zone(zone)?;
        Ok(format!("Warped to zone {zone}."))
    }

    /// Grants a research node whatever its prerequisites, cost, materials or
    /// required subject, through `grant_research` — the half of completion
    /// that follows the bill — so the log, notification and active-selection
    /// clearing are play's own.
    #[doc(hidden)]
    pub fn dev_unlock_research(&mut self, id: &ResearchId) -> Result<String, String> {
        let Some(def) = self
            .world
            .resource::<ResearchDb>()
            .get(id.as_str())
            .cloned()
        else {
            return Err(format!("No research called {id}."));
        };
        if self.node_researched(&def) {
            return Err(format!("{} is already researched.", def.name));
        }
        self.grant_research(&def);
        Ok(format!("Unlocked {}.", def.name))
    }
}
