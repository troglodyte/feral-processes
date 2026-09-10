//! Tool acquisition past the starter grant: `Game::forge_tool`,
//! `Game::install_tool`, `Game::uninstall_tool` — the routine acquisition
//! chain mirrored onto tools (spec decision 6). The catalogue and the slot
//! formula both of these read live in `crate::tools`; the act itself,
//! `Game::extract_program`, lives in `game/extraction.rs`.
//!
//! `Game::install_rig_tool` / `remove_rig_tool` are here rather than under
//! `game/base/` on purpose: the carrier a rig hands back and the carrier a
//! slot does *not* are the same object under two rules, and the two
//! functions in one file is what stops the next reader tidying them into
//! agreement.

use crate::components::{Hopper, Tools};
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
    ///
    /// **`remove_rig_tool` below deliberately does the opposite**, and the
    /// two sitting in one file is the whole defence against someone
    /// "fixing" the inconsistency. A slot holds knowledge the player cannot
    /// lose, so re-granting a carrier here would mint one out of nothing —
    /// and, since the starter tool is never spent to begin with, would make
    /// pulling it a way to print carriers. A rig holds the *object*: it was
    /// carried there and it is still there, so pulling it out hands back the
    /// thing that went in.
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

    /// The whole tool catalogue, sorted by id — `Game::perk_defs`' shape.
    /// Read by the renderer's width census, which has to measure what a
    /// shipped install can actually build rather than whatever a fixture
    /// happened to fit.
    pub fn tool_defs(&self) -> Vec<crate::tools::ToolDef> {
        self.world.resource::<ToolDb>().all().cloned().collect()
    }

    /// The rig's own tool screen — `Game::depot_filter_view`'s shape and its
    /// self-closing rule: `None` the moment `rig` stops being a standing rig
    /// beside the party, so a machine demolished or walked away from under
    /// the screen closes it rather than drawing a row that acts on nothing.
    ///
    /// Candidates are the tools the player is *carrying a carrier for*, not
    /// the tools they know: fitting one is a physical handover, and a row
    /// offering a tool with nothing behind it is a row whose only outcome is
    /// a refusal. `Routines` and `Gear` are filtered out here for the same
    /// reason `install_rig_tool` refuses them — a rig cannot run either.
    pub fn rig_tool_view(&self, rig: Entity) -> Option<crate::views::RigToolView> {
        if !self.adjacent_teardown_rigs().contains(&rig) {
            return None;
        }
        let kind = &self.world.get::<Structure>(rig)?.kind;
        let name = self
            .world
            .resource::<StructureDb>()
            .get(kind)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| kind.clone());
        let pos = self.world.get::<Position>(rig)?;
        let tile = (pos.x, pos.y);
        let hopper = self.world.get::<Hopper>(rig)?;
        let standing = hopper.standing_tool.clone();
        let queued = hopper.queue.len();
        let player = self.player_entity();
        let inventory = self.world.get::<Inventory>(player);
        let row = |def: &crate::tools::ToolDef| crate::views::RigToolRow {
            id: def.id.clone(),
            name: def.name.clone(),
            category: def.category,
            tier: def.tier,
            ticks: self.extraction_ticks(def),
            carriers_held: inventory
                .map(|inv| inv.count(&ItemId::tool(&def.id)))
                .unwrap_or(0),
        };
        let db = self.world.resource::<ToolDb>();
        let installed = standing
            .as_ref()
            .and_then(|id| db.get(id.as_str()))
            .map(row);
        let candidates = db
            .all()
            .filter(|def| self.rig_runs(def).is_ok())
            .filter(|def| standing.as_ref() != Some(&def.id))
            .filter(|def| {
                inventory
                    .map(|inv| inv.count(&ItemId::tool(&def.id)) > 0)
                    .unwrap_or(false)
            })
            .map(row)
            .collect();
        Some(crate::views::RigToolView {
            tile,
            name,
            installed,
            candidates,
            queued,
        })
    }

    /// Whether a rig can run `def` at all. The two categories it cannot are
    /// `Routines` and `Gear`, which is `Game::extract_program`'s own
    /// division rather than a second one: neither draws from `ToolDef::
    /// yields`, and what each produces — a routine written into a slot, a
    /// roll against the species' drop table — has nowhere to go in a
    /// machine's `Stock::output`.
    ///
    /// A `Result` rather than a `bool` so the refusal sentence is written
    /// once, beside the rule, instead of at the door and again at the
    /// filter that hides the row.
    fn rig_runs(&self, def: &crate::tools::ToolDef) -> Result<(), String> {
        match def.category {
            crate::tools::ToolCategory::Routines | crate::tools::ToolCategory::Gear => {
                Err(format!("The {} is work for your hands.", def.name))
            }
            _ => Ok(()),
        }
    }

    /// Spends one carrier of `tool` to fit it to `rig`, handing back the
    /// carrier of whatever was in it. **This is the one writer of
    /// `Hopper::standing_tool`**, which is what makes a rig's yield a
    /// property of the rig rather than of whichever tool the player
    /// happened to be holding when they last walked past.
    ///
    /// Refusals, in order, all before anything is spent: game-over or an
    /// active battle, no such rig beside the party, an id `ToolDb` cannot
    /// resolve, a category the rig cannot run, the tool already fitted, no
    /// carrier held.
    pub fn install_rig_tool(&mut self, rig: Entity, tool: &ToolId) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".to_string());
        }
        if !self.adjacent_teardown_rigs().contains(&rig) {
            return Err("There is no rig here to fit.".to_string());
        }
        let def = self
            .world
            .resource::<ToolDb>()
            .get(tool.as_str())
            .cloned()
            .ok_or_else(|| "Unknown tool.".to_string())?;
        self.rig_runs(&def)?;
        let fitted = self
            .world
            .get::<Hopper>(rig)
            .and_then(|h| h.standing_tool.clone());
        if fitted.as_ref() == Some(tool) {
            return Err(format!("{} is already fitted.", def.name));
        }
        let player = self.player_entity();
        let carrier = ItemId::tool(tool);
        if self
            .world
            .get::<Inventory>(player)
            .is_none_or(|inv| inv.count(&carrier) == 0)
        {
            return Err(format!("You're not carrying {}.", self.item_name(&carrier)));
        }

        self.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(carrier.clone(), 1);
        self.note_consumed(&carrier, 1, crate::base_ledger::ConsumeSource::Install);
        if let Some(old) = fitted {
            self.hand_back_rig_tool(&old);
        }
        // Re-stamped rather than left standing: every entry in the queue was
        // stamped from `standing_tool` when a body fetched it, so a stale
        // stamp is a body being stripped with a tool that is not in the
        // machine — which no line on any screen could explain.
        if let Some(mut hopper) = self.world.get_mut::<Hopper>(rig) {
            hopper.standing_tool = Some(tool.clone());
            for entry in hopper.queue.iter_mut() {
                entry.tool = tool.clone();
            }
        }
        self.log_base(format!("You fit the {} to the rig.", def.name));
        self.tick();
        Ok(())
    }

    /// Pulls the tool back out of `rig` and hands the carrier over. The rig
    /// runs nothing until another is fitted — `run_teardown_rigs` gates on
    /// `standing_tool` being set, so the programs already in the hopper stay
    /// put rather than draining on the tool that has left the building.
    ///
    /// Refusals, in order, all before anything is spent: game-over or an
    /// active battle, no such rig beside the party, nothing fitted.
    pub fn remove_rig_tool(&mut self, rig: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".to_string());
        }
        if !self.adjacent_teardown_rigs().contains(&rig) {
            return Err("There is no rig here to strip.".to_string());
        }
        let fitted = self
            .world
            .get::<Hopper>(rig)
            .and_then(|h| h.standing_tool.clone())
            .ok_or_else(|| "There is no tool fitted to it.".to_string())?;

        self.world.get_mut::<Hopper>(rig).unwrap().standing_tool = None;
        self.hand_back_rig_tool(&fitted);
        let name = self.tool_display_name(fitted.as_str());
        self.log_base(format!("You pull the {name} out of the rig."));
        self.tick();
        Ok(())
    }

    /// Puts a rig's tool back in the player's pack. The pack is unbounded —
    /// `Inventory` is a plain `Vec` with no capacity anywhere — so this
    /// cannot fail and there is no "lost" rung to write. That is the whole
    /// difference from `return_carried_program`, whose third rung exists
    /// because `DownedPrograms` **is** capped.
    fn hand_back_rig_tool(&mut self, tool: &ToolId) {
        self.grant_loot(ItemId::tool(tool), 1, LootSource::Refund);
    }

    /// Hands a destroyed rig's tool back, for the two paths a structure
    /// comes down by. Called beside `return_carried_program` at both, and
    /// for its reason: destroying the building must not destroy what was
    /// carried into it, and nothing fails to compile if one of the two
    /// sites is missed.
    pub(crate) fn return_rig_tool(&mut self, rig: Entity) {
        let Some(tool) = self
            .world
            .get_mut::<Hopper>(rig)
            .and_then(|mut h| h.standing_tool.take())
        else {
            return;
        };
        self.hand_back_rig_tool(&tool);
        let name = self.tool_display_name(tool.as_str());
        self.log_base(format!("You recover the {name} from the wreckage."));
    }
}
