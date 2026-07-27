//! Perk and research progression — what the player has unlocked and what
//! unlocking costs.

use crate::tuning::{
    ATTACKER_BONUS_PER_LEVEL, BUFFER_BONUS_PERCENT_PER_LEVEL, BUFFER_MIN_BONUS_PER_LEVEL,
    DEFENDER_BONUS_PER_LEVEL, EXPLOIT_FOCUS_BONUS_PER_LEVEL,
};
use crate::*;

impl Game {
    /// How many levels of `perk` the player has bought — 0 if none.
    pub fn player_perk_level(&self, perk: Perk) -> u32 {
        let player = self.player_entity();
        self.world
            .get::<Perks>(player)
            .map(|p| p.level(perk))
            .unwrap_or(0)
    }

    /// The player's effective Decompiler skill for decompile-chance
    /// calculations: their real `Decompiler` stat plus
    /// `EXPLOIT_FOCUS_BONUS_PER_LEVEL` for every level of `Perk::ExploitFocus`.
    pub(crate) fn player_decompiler_skill(&self) -> i32 {
        let player = self.player_entity();
        let base = self
            .world
            .get::<Decompiler>(player)
            .map(|d| d.skill)
            .unwrap_or(0);
        base + EXPLOIT_FOCUS_BONUS_PER_LEVEL * self.player_perk_level(Perk::ExploitFocus) as i32
    }

    /// The taming catalyst a decompile attempt would spend, paired with its
    /// `ItemDef::taming_potency`: whichever item in the player's inventory
    /// declares the highest potency. Which item that is comes purely from
    /// the item data, so a mod's stronger catalyst wins over a shipped one
    /// without any code knowing its id. Ties break on item id, so a stocked
    /// pair of equal catalysts always spends the same stack first. `None`
    /// when the player carries no catalyst at all — the single source of
    /// truth for "decompiling isn't available right now".
    pub(crate) fn taming_catalyst(&self) -> Option<(ItemId, f32)> {
        let db = self.world.resource::<ItemDb>();
        let inv = self.world.get::<Inventory>(self.player_entity())?;
        inv.items
            .iter()
            .filter(|(_, qty)| *qty > 0)
            .filter_map(|(id, _)| {
                db.get(id.as_str())
                    .and_then(|d| d.taming_potency)
                    .map(|potency| (id.clone(), potency))
            })
            .max_by(|a, b| {
                a.1.total_cmp(&b.1)
                    .then_with(|| b.0.as_str().cmp(a.0.as_str()))
            })
    }

    /// Spends Perk Points to buy another level of `perk` (see
    /// `perks::Perk`). Perks are repeatable — there's no cap on levels,
    /// only on how many Perk Points you've earned.
    pub fn unlock_perk(&mut self, perk: Perk) -> Result<(), String> {
        if self.is_game_over().is_some() {
            return Err("Can't do that right now.".into());
        }
        let player = self.player_entity();
        let level = {
            let mut perks = self
                .world
                .get_mut::<Perks>(player)
                .ok_or_else(|| "No perks available.".to_string())?;
            if perks.points < perk.cost() {
                return Err(format!(
                    "Not enough Perk Points (need {}, have {}).",
                    perk.cost(),
                    perks.points
                ));
            }
            perks.points -= perk.cost();
            perks.unlocked.push(perk);
            perks.level(perk)
        };
        match perk {
            Perk::Attacker => {
                if let Some(mut stats) = self.world.get_mut::<Stats>(player) {
                    stats.atk += ATTACKER_BONUS_PER_LEVEL;
                }
            }
            Perk::Defender => {
                if let Some(mut stats) = self.world.get_mut::<Stats>(player) {
                    stats.def += DEFENDER_BONUS_PER_LEVEL;
                }
            }
            Perk::Buffer => {
                if let Some(mut stats) = self.world.get_mut::<Stats>(player) {
                    let bonus = ((stats.max_hp as f32 * BUFFER_BONUS_PERCENT_PER_LEVEL).round()
                        as i32)
                        .max(BUFFER_MIN_BONUS_PER_LEVEL);
                    stats.max_hp += bonus;
                    stats.hp = stats.max_hp;
                }
            }
            _ => {}
        }
        self.log(format!(
            "You buy the {} perk (level {level}).",
            perk.display_name()
        ));
        Ok(())
    }

    pub fn is_researched(&self, id: &str) -> bool {
        self.world.resource::<Research>().0.contains(id)
    }

    /// Display names of `def`'s prerequisites that aren't unlocked yet, in
    /// the order the file lists them.
    pub(crate) fn missing_prereqs(&self, def: &ResearchDef) -> Vec<String> {
        let db = self.world.resource::<ResearchDb>();
        def.requires
            .iter()
            .filter(|id| !self.is_researched(id))
            .map(|id| {
                db.get(id)
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| id.clone())
            })
            .collect()
    }

    /// Every research node, ordered the way the menu shows them: available
    /// first, then locked, then already-unlocked, each group cheapest-first
    /// (see `ResearchDb::all`). Ordering lives here rather than in each
    /// renderer so both peers agree on what `[3]` means.
    pub fn research_nodes(&self) -> Vec<ResearchStatus> {
        let research_currency = self.research_currency();
        let held = self
            .world
            .get::<Inventory>(self.player_entity())
            .map(|inv| inv.count(&research_currency))
            .unwrap_or(0);
        let mut nodes: Vec<ResearchStatus> = self
            .world
            .resource::<ResearchDb>()
            .all()
            .map(|def| {
                let state = if self.is_researched(&def.id) {
                    ResearchState::Unlocked
                } else {
                    let missing = self.missing_prereqs(def);
                    if missing.is_empty() {
                        ResearchState::Available
                    } else {
                        ResearchState::Locked { missing }
                    }
                };
                ResearchStatus {
                    id: def.id.clone(),
                    name: def.name.clone(),
                    description: def.description.clone(),
                    cost: def.cost,
                    state,
                    affordable: held >= def.cost,
                }
            })
            .collect();
        // `sort_by_key` is stable, so cheapest-first survives inside each group.
        nodes.sort_by_key(|n| match n.state {
            ResearchState::Available => 0,
            ResearchState::Locked { .. } => 1,
            ResearchState::Unlocked => 2,
        });
        nodes
    }

    /// Unlocks `id`, consuming its Research Data cost. Fails with an
    /// explicit message when the id is unknown, it's already unlocked, a
    /// prerequisite is missing, or the player can't pay.
    pub fn unlock_research(&mut self, id: &str) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let def = self
            .world
            .resource::<ResearchDb>()
            .get(id)
            .cloned()
            .ok_or_else(|| "Unknown research.".to_string())?;
        if self.is_researched(id) {
            return Err(format!("{} is already researched.", def.name));
        }
        let missing = self.missing_prereqs(&def);
        if !missing.is_empty() {
            return Err(format!("Requires {} first.", missing.join(", ")));
        }
        let player = self.player_entity();
        let research_currency = self.research_currency();
        let held = self
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(&research_currency);
        if held < def.cost {
            return Err(format!("Not enough Research Data ({held}/{}).", def.cost));
        }
        self.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(research_currency, def.cost);
        self.world
            .resource_mut::<Research>()
            .0
            .insert(def.id.clone());
        self.log(format!("Research complete: {}.", def.name));
        Ok(())
    }
}
