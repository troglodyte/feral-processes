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

    /// Adds one random species from what can spawn on the player's current
    /// tile as a companion. The pool is `habitat_pools` with the field
    /// spawner's own depth (the party's Stack depth, `None` on the surface),
    /// sorted by id and drawn with the seeded
    /// game RNG; the adoption mirrors `deliver_offer`'s `Program` arm, the
    /// one path that hands the player a companion from nothing.
    #[doc(hidden)]
    pub fn dev_spawn_companion(&mut self) -> Result<String, String> {
        if self.roster_room() == 0 {
            return Err("Your roster is full.".into());
        }
        let at = *self
            .world
            .get::<Position>(self.player_entity())
            .ok_or("The player has no tile.")?;
        let depth = self.stack_pos().map(|pos| pos.depth);
        let Some((mut pool, _)) = self.habitat_pools(at.x, at.y, depth, 0) else {
            return Err("Nothing can spawn on this tile.".into());
        };
        if pool.is_empty() {
            return Err("Nothing can spawn on this tile.".into());
        }
        pool.sort();
        let species = {
            let mut rng = self.world.resource_mut::<GameRng>();
            pool[rng.0.random_range(0..pool.len())].clone()
        };
        let mult = self.world.resource::<ZoneLevel>().stat_multiplier() as f32;
        let anchor = self.anchor_position().unwrap_or((0, 0));
        match self.adopt_program(&species, anchor.0, anchor.1, mult) {
            Some(program) => Ok(format!("{} is yours", self.creature_label(program))),
            None => Err(format!("{species} would not come.")),
        }
    }
}
