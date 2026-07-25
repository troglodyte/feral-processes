//! What a won fight pays out: equipment drops, loot, experience, and
//! decompiling a defeated program into a companion.

use crate::*;

impl Game {
    /// Every gear drop `species` can roll, from both directions the schema
    /// allows it to be declared: the species' own `equipment_drop`, plus
    /// every item whose `droppable` names this species. An item declared on
    /// both sides is rolled once at the better chance rather than twice.
    /// Sorted by item id so a seeded run always consumes its rolls in the
    /// same order.
    pub(crate) fn equipment_drops_for(&self, species: &SpeciesDef) -> Vec<(ItemId, f32)> {
        let mut drops: Vec<(ItemId, f32)> = species.equipment_drop.iter().cloned().collect();
        for def in self.world.resource::<ItemDb>().all() {
            let Some(sources) = &def.droppable else {
                continue;
            };
            for chance in sources
                .iter()
                .filter(|(id, _)| *id == species.id)
                .map(|&(_, chance)| chance)
            {
                match drops.iter_mut().find(|(id, _)| *id == def.id) {
                    Some(existing) => existing.1 = existing.1.max(chance),
                    None => drops.push((def.id.clone(), chance)),
                }
            }
        }
        drops.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
        drops
    }

    /// Defeated (not tamed) rogue programs drop whatever resource their
    /// species is associated with, if any — the same `work_resource` used
    /// to decide what a tamed member of that species can gather.
    pub(crate) fn award_loot(&mut self, wild: Entity) {
        let Some(species_id) = self.world.get::<Creature>(wild).map(|c| c.species.clone()) else {
            return;
        };
        let Some(species) = self.world.resource::<SpeciesDb>().get(&species_id).cloned() else {
            return;
        };

        if let Some(resource) = &species.work_resource {
            let qty = {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_range(1..=2)
            };
            let landed = self.grant_loot(resource.clone(), qty);
            if landed > 0 {
                self.log_kind(
                    MessageKind::Loot,
                    format!("It drops {} {}.", landed, self.item_name(resource)),
                );
            }
        }

        for (item, chance) in self.equipment_drops_for(&species) {
            let roll = {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_bool(chance.clamp(0.0, 1.0) as f64)
            };
            if roll && self.grant_loot(item.clone(), 1) > 0 {
                self.log_kind(
                    MessageKind::Loot,
                    format!("It also drops a {}!", self.item_name(&item)),
                );
            }
        }

        if species.is_boss {
            let qty = {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_range(BOSS_PORTAL_FRAGMENT_DROP)
            };
            let landed = self.grant_loot(self.craft_currency(), qty);
            if landed > 0 {
                self.log_kind(
                    MessageKind::Loot,
                    format!("Its crash leaves behind a cache of {landed} portal fragments!"),
                );
            }
        } else {
            let portal_fragment_roll = {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_bool(PORTAL_FRAGMENT_DROP_CHANCE)
            };
            if portal_fragment_roll && self.grant_loot(self.craft_currency(), 1) > 0 {
                self.log_kind(MessageKind::Loot, "It leaves behind a portal fragment.");
            }
        }
    }

    /// Awards `amount` XP to the player, growing stats and fully healing on
    /// any level-up gained, then awards every current party member half as
    /// much (see `award_party_xp`) — fighting beside you pays off even on
    /// rounds where only the player's hit actually lands. Silently does
    /// nothing for the player if they're somehow missing an `Experience`
    /// component (shouldn't happen in practice).
    pub(crate) fn award_player_xp(&mut self, player: Entity, amount: u32) {
        let (levels, new_level) = {
            let mut query = self.world.query::<(&mut Experience, &mut Stats)>();
            let Ok((mut exp, mut stats)) = query.get_mut(&mut self.world, player) else {
                return;
            };
            let levels = progression::add_xp(
                &mut exp,
                &mut stats,
                amount,
                progression::BASELINE_GROWTH_MULTIPLIER,
                // The player has no level ceiling — only creatures do.
                None,
            );
            (levels, exp.level)
        };
        if levels > 0 {
            if let Some(mut decompiler) = self.world.get_mut::<Decompiler>(player) {
                decompiler.skill += DECOMPILER_SKILL_PER_LEVEL * levels as i32;
            }
            if let Some(mut perks) = self.world.get_mut::<Perks>(player) {
                perks.points += PERK_POINTS_PER_LEVEL * levels;
            }
            self.log_kind(
                MessageKind::LevelUp,
                format!("You gain {amount} XP and reach level {new_level}!"),
            );
        } else {
            self.log(format!("You gain {amount} XP."));
        }
        self.award_party_xp(amount / PARTY_XP_DIVISOR);
    }

    /// Awards `amount` XP to every program in the active party (see
    /// `resources::Party`), each independently able to level up from it —
    /// the party-wide, half-rate companion to `award_player_xp`. A no-op
    /// for any party member somehow missing `Experience` (shouldn't happen
    /// in practice) or if the party is empty. Only logs a level-up, not
    /// every ordinary gain, so a busy fight doesn't flood the feed with a
    /// line per party member per kill.
    pub(crate) fn award_party_xp(&mut self, amount: u32) {
        if amount == 0 {
            return;
        }
        let party = self.world.resource::<Party>().0.clone();
        for companion in party {
            let species_growth = self
                .world
                .get::<Creature>(companion)
                .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
                .map(|s| s.growth_multiplier)
                .unwrap_or(progression::BASELINE_GROWTH_MULTIPLIER);
            let individual_roll = self
                .world
                .get::<Potential>(companion)
                .map(|p| p.growth_roll)
                .unwrap_or(Potential::NEUTRAL.growth_roll);
            let growth_multiplier = species_growth * individual_roll;
            let leveled = {
                let mut query = self.world.query::<(&mut Experience, &mut Stats)>();
                let Ok((mut exp, mut stats)) = query.get_mut(&mut self.world, companion) else {
                    continue;
                };
                progression::add_xp(
                    &mut exp,
                    &mut stats,
                    amount,
                    growth_multiplier,
                    Some(progression::CREATURE_MAX_LEVEL),
                ) > 0
            };
            if leveled {
                let name = self.creature_label(companion);
                let level = self.world.get::<Experience>(companion).unwrap().level;
                self.log_kind(
                    MessageKind::LevelUp,
                    format!("{name} gains {amount} XP and levels up to {level}!"),
                );
            }
        }
    }

    /// One decompile attempt against `group`'s front program: spends a
    /// catalyst, rolls `taming::capture_chance`, and on success converts the
    /// target into a tamed program and drops it from the group.
    ///
    /// `None` means the attempt was refused before anything was spent — no
    /// catalyst, or no room on the roster — so the caller must not charge a
    /// turn for it. `Some(battle_over)` means the attempt happened.
    pub(crate) fn attempt_decompile(&mut self, group: usize, player: Entity) -> Option<bool> {
        // Refuse before spending the catalyst (or the turn) if the roster is
        // already full — a captured program has to live somewhere.
        let capacity = self.pet_capacity();
        let owned = self.pet_count();
        if owned >= capacity {
            self.log(format!(
                "Your roster is full ({owned}/{capacity}) — sell a program at a Market, fuse two together, or deploy a Data Cache to make room."
            ));
            return None;
        }

        let Some((catalyst, potency)) = self.taming_catalyst() else {
            self.log("You have no taming catalyst.");
            return None;
        };
        let front = self.front_of_group(group)?;
        self.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(catalyst, 1);

        let (hp_fraction, species_id) = {
            let stats = *self.world.get::<Stats>(front).unwrap();
            let species = self.world.get::<Creature>(front).unwrap().species.clone();
            (stats.hp_fraction(), species)
        };
        let taming_difficulty = self
            .world
            .resource::<SpeciesDb>()
            .get(&species_id)
            .map(|s| s.taming_difficulty)
            .unwrap_or(0.5);
        let decompiler_skill = self.player_decompiler_skill();
        let chance =
            taming::capture_chance(hp_fraction, potency, taming_difficulty, decompiler_skill);
        let roll = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(chance as f64)
        };

        if !roll {
            self.log("The program's ICE holds — decompile failed!");
            return Some(false);
        }

        let wild_max_hp = self.world.get::<Stats>(front).unwrap().max_hp;
        let nest = self.world.get::<NestGuardian>(front).map(|g| g.nest);
        self.world
            .entity_mut(front)
            .remove::<(Hostile, WanderAi, NestGuardian)>();
        self.world
            .entity_mut(front)
            .insert((Tamed { owner: player }, Experience::default()));
        if let Some(nest) = nest
            && let Some(mut n) = self.world.get_mut::<Nest>(nest)
        {
            n.pending_respawns.push(NEST_RESPAWN_TICKS);
        }
        self.log("ICE breached! The program now runs under your control.");
        self.award_player_xp(player, wild_max_hp as u32);
        if self.remove_member(group, 0) {
            self.end_battle(player, Some(front));
            return Some(true);
        }
        self.log("Another rogue program from the pack engages!");
        Some(false)
    }
}
