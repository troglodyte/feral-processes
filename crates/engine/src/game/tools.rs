//! Tool acquisition past the starter grant: `Game::forge_tool`, and this
//! phase's remaining task, `Game::install_tool`/`Game::uninstall_tool` —
//! the routine acquisition chain mirrored onto tools (spec decision 6).
//! The catalogue and the slot formula both of these read live in
//! `crate::tools`; the act itself, `Game::extract_program`, lives in
//! `game/extraction.rs`.

use crate::tools::{ToolDb, ToolId};
use crate::*;

impl Game {
    /// Burns the def's `forge_cost` to grant one carrier of `tool` —
    /// `etch_disk`'s own order, materials-then-item, since knowing a tool
    /// is not enough on its own to make one (spec section 2 table).
    ///
    /// Requires no structure — `etch_disk` requires none either, and spec
    /// decision 7 keeps the whole feature structure-free until phase 3.
    /// Every refusal lands before anything is spent: game-over or an
    /// active battle, an id `ToolDb` cannot resolve, a tool the player
    /// hasn't researched, then a cost they cannot pay.
    pub fn forge_tool(&mut self, tool: &ToolId) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".to_string());
        }
        let def = self
            .world
            .resource::<ToolDb>()
            .get(tool.as_str())
            .cloned()
            .ok_or_else(|| "Unknown tool.".to_string())?;
        if !self.knows_tool(tool) {
            return Err(format!("You haven't researched the {}.", def.name));
        }
        let player = self.player_entity();
        {
            let inventory = self.world.get::<Inventory>(player).unwrap();
            if let Some((item, qty)) = def
                .forge_cost
                .iter()
                .find(|(item, qty)| inventory.count(item) < *qty)
            {
                return Err(format!(
                    "Not enough {} ({}/{}).",
                    self.item_name(item),
                    inventory.count(item),
                    qty
                ));
            }
        }
        for (item, qty) in &def.forge_cost {
            self.world
                .get_mut::<Inventory>(player)
                .unwrap()
                .take(item.clone(), *qty);
            self.note_consumed(item, *qty, crate::base_ledger::ConsumeSource::Craft);
        }
        self.grant_loot(ItemId::tool(tool), 1, LootSource::Forge);
        self.log(format!("You forge a {}.", def.name));
        Ok(())
    }
}
