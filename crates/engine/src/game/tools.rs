//! Tool acquisition past the starter grant: `Game::forge_tool`,
//! `Game::install_tool`, `Game::uninstall_tool` — the routine acquisition
//! chain mirrored onto tools (spec decision 6). The catalogue and the slot
//! formula both of these read live in `crate::tools`; the act itself,
//! `Game::extract_program`, lives in `game/extraction.rs`.

use crate::components::Tools;
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
    /// hasn't researched, an already-installed tool, then a cost they
    /// cannot pay.
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
        // A carrier `install_tool` will always refuse — `etch_disk` allows
        // this (a second disk can still go on a companion), but the player
        // is the only tool holder, so a second carrier of an installed tool
        // has nowhere to go.
        if self
            .world
            .get::<Tools>(player)
            .is_some_and(|t| t.0.contains(tool))
        {
            return Err(format!("{} is already installed.", def.name));
        }
        // Folded into a per-item total before the check: a modded
        // `forge_cost` naming one item on two lines must be checked (and
        // spent) against their sum, or each line passes independently
        // against a total neither alone asked for and `Inventory::take`'s
        // own `min` silently under-charges the second line.
        let mut cost: std::collections::BTreeMap<&ItemId, u32> = std::collections::BTreeMap::new();
        for (item, qty) in &def.forge_cost {
            *cost.entry(item).or_insert(0) += qty;
        }
        {
            let inventory = self.world.get::<Inventory>(player).unwrap();
            if let Some((item, qty)) = cost
                .iter()
                .find(|(item, qty)| inventory.count(item) < **qty)
            {
                return Err(format!(
                    "Not enough {} ({}/{}).",
                    self.item_name(item),
                    inventory.count(item),
                    qty
                ));
            }
        }
        for (item, qty) in &cost {
            self.world
                .get_mut::<Inventory>(player)
                .unwrap()
                .take((*item).clone(), *qty);
            self.note_consumed(item, *qty, crate::base_ledger::ConsumeSource::Craft);
        }
        self.grant_loot(ItemId::tool(tool), 1, LootSource::Forge);
        self.log(format!("You forge a {}.", def.name));
        Ok(())
    }

    /// `id`'s display name, or the raw id for one `ToolDb` cannot resolve —
    /// `ability_display_name`'s analog, needed by `uninstall_tool`, which
    /// has only the id left once the slot is cleared.
    fn tool_display_name(&self, id: &str) -> String {
        self.world
            .resource::<ToolDb>()
            .get(id)
            .map(|t| t.name.clone())
            .unwrap_or_else(|| id.to_string())
    }

    /// Spends one carrier of `tool` to write it into the player's next free
    /// slot. The player is the only tool holder, so there is no entity
    /// argument and no `owns_routine_holder` rung — `install_disk`'s shape
    /// with that one rung removed.
    ///
    /// Refusals, in order, all before anything is spent: game-over or an
    /// active battle, an id `ToolDb` cannot resolve, the player cannot hold
    /// tools at all, the tool is already installed, no free slot
    /// (`installed.len() >= tools::player_tool_slots(level)`), no carrier
    /// held.
    pub fn install_tool(&mut self, tool: &ToolId) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".to_string());
        }
        let def = self
            .world
            .resource::<ToolDb>()
            .get(tool.as_str())
            .cloned()
            .ok_or_else(|| "Unknown tool.".to_string())?;
        let player = self.player_entity();
        // `install_disk`'s own shape: refused here, before the spend, not
        // read tolerantly and then `unwrap()`ed on the write below — the
        // player always spawns with `Tools`, so this is unreachable today,
        // but the write past the spend must not be the first place that
        // would find out otherwise.
        let installed = self
            .world
            .get::<Tools>(player)
            .map(|t| t.0.clone())
            .ok_or_else(|| "That can't hold tools.".to_string())?;
        if installed.contains(tool) {
            return Err(format!("{} is already installed.", def.name));
        }
        let level = self.world.get::<Experience>(player).unwrap().level;
        if installed.len() >= crate::tools::player_tool_slots(level) {
            return Err("There's no free tool slot — pull one out first.".to_string());
        }
        let carrier = ItemId::tool(tool);
        if self.world.get::<Inventory>(player).unwrap().count(&carrier) == 0 {
            return Err(format!("You're not carrying {}.", self.item_name(&carrier)));
        }
        self.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(carrier.clone(), 1);
        self.note_consumed(&carrier, 1, crate::base_ledger::ConsumeSource::Install);
        self.world
            .get_mut::<Tools>(player)
            .unwrap()
            .0
            .push(tool.clone());
        self.log(format!("You install the {}.", def.name));
        Ok(())
    }

    /// Frees `slot`. What is in the slot *is* the tool — `install_disk`'s
    /// rule — so this hands back no carrier; the player keeps only the
    /// knowledge, which they never lost.
    pub fn uninstall_tool(&mut self, slot: usize) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".to_string());
        }
        let player = self.player_entity();
        let mut installed = self
            .world
            .get::<Tools>(player)
            .map(|t| t.0.clone())
            .unwrap_or_default();
        if slot >= installed.len() {
            return Err("That slot is empty.".to_string());
        }
        let tool = installed.remove(slot);
        self.world.entity_mut(player).insert(Tools(installed));
        let name = self.tool_display_name(tool.as_str());
        self.log(format!("You pull the {name} tool."));
        Ok(())
    }
}
