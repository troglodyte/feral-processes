//! Perk and research progression — what the player has unlocked and what
//! unlocking costs.

use crate::game::inspection::power_ratio;
use crate::taming::{DecompilerBonuses, TargetResistance};
use crate::tuning::DEFAULT_TAMING_DIFFICULTY;
use crate::*;

impl Game {
    /// How many levels of `perk` the player has bought — 0 if none.
    pub fn player_perk_level(&self, perk: Perk) -> u32 {
        self.player_perks().map(|p| p.level(perk)).unwrap_or(0)
    }

    /// The player's bought perks, for the `perks::` queries to read.
    ///
    /// `None` rather than a default is what those queries expect: every one
    /// of them answers 0 for an entity that has never bought anything, so
    /// no caller needs a branch of its own.
    pub(crate) fn player_perks(&self) -> Option<&Perks> {
        self.world.get::<Perks>(self.player_entity())
    }

    /// Everything the player brings to a decompile attempt: their real
    /// `Decompiler` stat (levels plus equipment), whatever
    /// `Perk::ExploitFocus` has bought off the target's HP penalty, and any
    /// running `CaptureBoost` field buff. The one place all three are
    /// assembled — all three `taming::capture_chance` call sites go through
    /// here, so the odds a player is shown and the odds they are rolled
    /// against cannot drift apart.
    pub(crate) fn player_decompiler_bonuses(&self) -> DecompilerBonuses {
        let player = self.player_entity();
        DecompilerBonuses {
            skill: self
                .world
                .get::<Decompiler>(player)
                .map(|d| d.skill)
                .unwrap_or(0),
            hp_penalty_reduction: crate::perks::decompile_hp_penalty_reduction(self.player_perks()),
            capture_boost_pct: self.field_buff_power(player, FieldBuffKind::CaptureBoost),
        }
    }

    /// Everything the target brings to a decompile attempt: its remaining
    /// Integrity, its species' resistance, how many attempts this fight has
    /// already spent on it, and how its `Stats::power` compares to the
    /// player's. The mirror of `player_decompiler_bonuses` above and the one
    /// place these four are assembled, for the same reason — the two call
    /// sites that *show* odds and the one that *rolls* them must not drift
    /// apart, and `prior_attempts` is exactly the sort of term a display
    /// would forget.
    ///
    /// A species the `SpeciesDb` doesn't know falls back to
    /// `DEFAULT_TAMING_DIFFICULTY` rather than refusing, which is what the
    /// battle view already did.
    pub(crate) fn target_resistance(&self, entity: Entity) -> Option<TargetResistance> {
        let stats = self.world.get::<Stats>(entity)?;
        Some(TargetResistance {
            hp_fraction: stats.hp_fraction(),
            taming_difficulty: self
                .world
                .get::<Creature>(entity)
                .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
                .map(|s| s.taming_difficulty)
                .unwrap_or(DEFAULT_TAMING_DIFFICULTY),
            prior_attempts: self
                .world
                .get_resource::<BattleState>()
                .and_then(|b| b.decompile_attempts.get(&entity).copied())
                .unwrap_or(0),
            power_ratio: power_ratio(stats.power(), self.player_power()) as f32,
        })
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

    /// Drains banked overflow XP into Perk Points, returning how many were
    /// minted.
    ///
    /// XP earned at the level cap accumulates in `Experience::xp` (see
    /// `progression::add_xp`) instead of being discarded. This is the only
    /// thing that spends it, and only for the player — a companion has no
    /// `Perks` at all, so its overflow just sits there, which is the
    /// behaviour every creature had before the cap existed.
    ///
    /// The price rises with perks already held, `OVERFLOW_XP_STEP`'s reason:
    /// flat, this is an unbounded linear power source. It re-reads the price
    /// after every point, so one call cannot mint a run's worth at the
    /// opening rate.
    pub(crate) fn convert_overflow_xp(&mut self) -> u32 {
        let player = self.player_entity();
        let mut minted = 0;
        loop {
            let held = match self.world.get::<Perks>(player) {
                Some(perks) => perks.unlocked.len() as u32,
                None => return minted,
            };
            let price =
                crate::tuning::OVERFLOW_XP_BASE + crate::tuning::OVERFLOW_XP_STEP * (held + minted);
            let Some(mut exp) = self.world.get_mut::<Experience>(player) else {
                return minted;
            };
            if exp.xp < price {
                return minted;
            }
            exp.xp -= price;
            minted += 1;
            if let Some(mut perks) = self.world.get_mut::<Perks>(player) {
                perks.points += 1;
            }
        }
    }

    /// Spends Perk Points to buy another level of `perk` (see
    /// `perks::Perk`). Perks are repeatable — there's no cap on levels,
    /// only on how many Perk Points you've earned.
    ///
    /// Name and price come from `PerkDb`, so a perk whose `.ron` file is
    /// missing or malformed can't be bought at all — the same state the
    /// picker shows by leaving it out of the list.
    pub fn unlock_perk(&mut self, perk: Perk) -> Result<(), String> {
        if self.is_game_over().is_some() {
            return Err("Can't do that right now.".into());
        }
        let player = self.player_entity();
        let (name, cost) = {
            let def = self
                .world
                .resource::<PerkDb>()
                .get(perk)
                .ok_or_else(|| "That perk isn't available.".to_string())?;
            (def.name.clone(), def.cost)
        };
        let level = {
            let mut perks = self
                .world
                .get_mut::<Perks>(player)
                .ok_or_else(|| "No perks available.".to_string())?;
            if perks.points < cost {
                return Err(format!(
                    "Not enough Perk Points (need {cost}, have {}).",
                    perks.points
                ));
            }
            perks.points -= cost;
            perks.unlocked.push(perk);
            perks.level(perk)
        };
        // What the gain *is* belongs to the perk; writing it belongs here,
        // so `Stats` keeps one writer. `Buffer` reads the current maximum,
        // which is why the gain is asked for after the purchase.
        let max_hp = self
            .world
            .get::<Stats>(player)
            .map(|s| s.max_hp)
            .unwrap_or(0);
        if let Some(gain) = crate::perks::purchase_stat_gain(perk, max_hp)
            && let Some(mut stats) = self.world.get_mut::<Stats>(player)
        {
            match gain {
                crate::perks::StatGain::Atk(n) => stats.atk += n,
                crate::perks::StatGain::Mitigation(n) => stats.mitigation += n,
                crate::perks::StatGain::MaxHp(n) => {
                    stats.max_hp += n;
                    stats.hp = stats.max_hp;
                }
            }
        }
        self.log(format!("You buy the {name} perk (level {level})."));
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

    /// The zone `def` is still waiting on, or `None` if the party has already
    /// reached it. Read the same way `Game::upgrade_ceiling` reads it, and
    /// the one definition of the gate — `research_nodes` explains it and
    /// `unlock_research` refuses on it, so a menu cannot promise a node the
    /// purchase would turn down.
    fn research_zone_gate(&self, def: &ResearchDef) -> Option<u32> {
        (def.min_zone > self.world.resource::<ZoneLevel>().0).then_some(def.min_zone)
    }

    /// Every research node, ordered the way the menu shows them: available
    /// first, then locked, then already-unlocked, each group cheapest-first
    /// (see `ResearchDb::all`). Ordering lives here rather than in each
    /// renderer so both peers agree on what `[3]` means.
    ///
    /// A zone-gated node is `Locked` rather than filtered out, for the reason
    /// `Game::upgrade_ceiling` records about a structure stalled at its zone
    /// ceiling: hiding the stalled rows would mean a player who never
    /// breached never learns the tier is there, and the visible band is
    /// exactly what makes breaching worth doing.
    pub fn research_nodes(&self) -> Vec<ResearchStatus> {
        let research_currency = self.research_currency();
        let held = self
            .world
            .get::<Inventory>(self.player_entity())
            .map(|inv| inv.count(&research_currency))
            .unwrap_or(0);
        let recommended = self.world.resource::<ResearchDb>().recommended_ids();
        let mut nodes: Vec<ResearchStatus> = self
            .world
            .resource::<ResearchDb>()
            .all()
            .map(|def| {
                let state = if self.is_researched(&def.id) {
                    ResearchState::Unlocked
                } else {
                    let missing = self.missing_prereqs(def);
                    let min_zone = self.research_zone_gate(def);
                    if missing.is_empty() && min_zone.is_none() {
                        ResearchState::Available
                    } else {
                        ResearchState::Locked { missing, min_zone }
                    }
                };
                ResearchStatus {
                    id: def.id.clone(),
                    name: def.name.clone(),
                    description: def.description.clone(),
                    cost: def.cost,
                    state,
                    affordable: held >= def.cost,
                    recommended: recommended.contains(&def.id),
                    #[cfg(test)]
                    unlocks_abilities: def.unlocks_abilities.clone(),
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
    /// prerequisite is missing, the party hasn't reached the node's zone, or
    /// the player can't pay.
    ///
    /// The zone is checked **before the cost** for the reason
    /// `upgrade_structure` checks its ceilings before materials: a player at
    /// zone 1 looking at a zone-3 node must be told about the zone, not sent
    /// to earn Research Data they could not have spent. It is checked
    /// **after** the prereqs, which is the existing order extended rather
    /// than rearranged.
    ///
    /// The gate is on buying, not on having: `resources::Research` holds what
    /// has already been unlocked and is never re-validated, so a save written
    /// before this gate existed keeps every node it paid for whatever zone
    /// the party is standing in.
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
        if let Some(zone) = self.research_zone_gate(&def) {
            return Err(format!("Requires Zone {zone} first."));
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
        // Knowledge, not items: what a node hands over is the ability to
        // write this routine onto a blank disk the base has to manufacture.
        for ability in &def.unlocks_abilities {
            let name = self.ability_display_name(ability);
            let fresh = self
                .world
                .resource_mut::<KnownRoutines>()
                .0
                .insert(ability.clone());
            if fresh {
                self.log(format!("You learn the {name} routine."));
            }
        }
        Ok(())
    }
}
