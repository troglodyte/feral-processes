//! What a won fight pays out: equipment drops, loot, experience, and
//! decompiling a defeated program into a companion.

use crate::tuning::{
    DECOMPILER_SKILL_PER_LEVEL, NEST_RESPAWN_TICKS, PARTY_XP_DIVISOR, PERK_POINTS_PER_LEVEL,
    STACK_BOSS_PORTAL_FRAGMENT_DROP, SURFACE_BOSS_LOOT_BAND_FLOOR_PERCENT, SURFACE_BOSS_LOOT_DROPS,
    SURFACE_BOSS_LOOT_VALUE_PER_ZONE,
};
use crate::tuning::{DEFAULT_TAMING_DIFFICULTY, WORK_RESOURCE_DROP};
use crate::*;

impl Game {
    /// Every gear drop `species` can roll, from both directions the schema
    /// allows it to be declared: the species' own `equipment_drop`, plus
    /// every item whose `droppable` names this species. An item declared on
    /// both sides is rolled once at the better chance rather than twice.
    /// Sorted by item id so a seeded run always consumes its rolls in the
    /// same order.
    ///
    /// A running `DropBoost` field buff scales every chance here by its
    /// power, last — so it applies uniformly regardless of which side of
    /// the schema a drop came from. The result can run past 1.0; the one
    /// caller, `award_loot`, already clamps before rolling, so this leaves
    /// it unclamped rather than duplicating that.
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
        let boost_pct = self.field_buff_power(self.player_entity(), FieldBuffKind::DropBoost);
        if boost_pct != 0 {
            let multiplier = 1.0 + boost_pct as f32 / 100.0;
            for (_, chance) in &mut drops {
                *chance *= multiplier;
            }
        }
        drops
    }

    /// Defeated (not tamed) rogue programs drop whatever resource their
    /// species is associated with, if any.
    ///
    /// `SpeciesDef::work_resource` does *not* decide what a tamed member of
    /// that species gathers, despite the name — a cronjob's output comes
    /// from the structure's `produces`, and any species can work any
    /// structure. Its only other reader is the inspection view. So changing
    /// a species' `work_resource` changes what killing it drops and nothing
    /// else.
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
                rng.0.random_range(WORK_RESOURCE_DROP)
            };
            let landed = self.grant_loot(resource.clone(), qty);
            if landed > 0 {
                self.log_kind(
                    MessageKind::Loot,
                    format!("It drops {} {}.", landed, self.item_name_tagged(resource)),
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
                    format!("It also drops a {}!", self.item_name_tagged(&item)),
                );
            }
        }

        // Underground this is the stack's guardian going down, and the one
        // point that knows it actually died rather than being fled from.
        self.mark_lair_cleared();

        // Same "it actually died" guarantee, spent on the other thing that
        // needs it. `raise_trace` no-ops on the surface, which is where the
        // overwhelming majority of these calls come from.
        self.raise_trace(crate::tuning::TRACE_PER_KILL);

        if species.is_boss {
            // Third consumer of the same "it actually died" guarantee. The
            // record is all that happens here: what it earned is
            // `achievement_system`'s to decide, in this same tick.
            self.world
                .resource_mut::<crate::resources::RunFeats>()
                .bosses_defeated
                .push(species_id.clone());

            match self.stack_pos() {
                Some(pos) => self.pay_stack_boss_fragments(pos.depth),
                None => self.pay_surface_boss_gear(),
            }
        }
    }

    /// The breaching currency, and the only place in the game that pays it
    /// (`STACK_BOSS_PORTAL_FRAGMENT_DROP`). Reached only from `award_loot`'s
    /// boss branch while `Locale::Stack` is live, where a boss can only be a
    /// lair guardian — so this is what the party went down for.
    fn pay_stack_boss_fragments(&mut self, depth: u32) {
        let qty = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(STACK_BOSS_PORTAL_FRAGMENT_DROP) * depth
        };
        let landed = self.grant_loot(self.craft_currency(), qty);
        if landed > 0 {
            self.log_kind(
                MessageKind::Loot,
                format!("Its crash leaves behind a cache of {landed} portal fragments!"),
            );
        }
    }

    /// What a boss killed on the surface pays instead: gear from
    /// `surface_boss_loot`'s zone band, on top of the species' own
    /// `equipment_drops_for` rolls that every kill gets.
    ///
    /// Drawn with replacement, so a zone whose band happens to be thin pays
    /// the same *number* of items as a rich one — a boss is a wall wherever
    /// it is met, and the band already says how good the items are.
    fn pay_surface_boss_gear(&mut self) {
        let pool = self.surface_boss_loot();
        if pool.is_empty() {
            return;
        }
        for _ in 0..SURFACE_BOSS_LOOT_DROPS {
            let item = {
                let mut rng = self.world.resource_mut::<GameRng>();
                pool[rng.0.random_range(0..pool.len())].clone()
            };
            if self.grant_loot(item.clone(), 1) > 0 {
                self.log_kind(
                    MessageKind::Loot,
                    format!("Its crash spills a {}!", self.item_name_tagged(&item)),
                );
            }
        }
    }

    /// The pool a defeated surface boss draws from: every equippable item
    /// whose `ItemDef::value` sits in this zone's band, which is
    /// `SURFACE_BOSS_LOOT_VALUE_PER_ZONE` per zone wide at the top and
    /// `SURFACE_BOSS_LOOT_BAND_FLOOR_PERCENT` of that at the bottom.
    ///
    /// Derived from `value` rather than a new schema field, so a modded item
    /// joins the pool by existing and the ladder documented in
    /// `assets/items/README.md` is the single place a tier is declared. The
    /// equipment filter is what keeps non-gear that happens to be worth the
    /// same — an Access Shard is worth 12, exactly a Hardened Shell — out of
    /// a payout that is supposed to make the party stronger.
    ///
    /// A band that selects nothing falls back to the best gear there is
    /// rather than paying nothing: the ceiling climbs forever but the ladder
    /// does not, so a deep enough run would otherwise walk off the top of it.
    ///
    /// Sorted by id so a seeded run consumes its draws in the same order
    /// however the item files happen to load — the same guarantee
    /// `equipment_drops_for` and `open_cache` make.
    pub(crate) fn surface_boss_loot(&self) -> Vec<ItemId> {
        let zone = self.world.resource::<ZoneLevel>().0;
        let ceiling = SURFACE_BOSS_LOOT_VALUE_PER_ZONE.saturating_mul(zone);
        let floor = ceiling * SURFACE_BOSS_LOOT_BAND_FLOOR_PERCENT / 100;

        let gear: Vec<(ItemId, u32)> = self
            .world
            .resource::<ItemDb>()
            .all()
            .filter(|def| def.equipment.is_some())
            .map(|def| {
                (
                    def.id.clone(),
                    def.value.unwrap_or(crate::tuning::DEFAULT_ITEM_VALUE),
                )
            })
            .collect();
        let mut pool: Vec<ItemId> = gear
            .iter()
            .filter(|(_, value)| (floor..=ceiling).contains(value))
            .map(|(id, _)| id.clone())
            .collect();
        if pool.is_empty() {
            let best = gear.iter().map(|&(_, value)| value).max().unwrap_or(0);
            pool = gear
                .iter()
                .filter(|&&(_, value)| value == best)
                .map(|(id, _)| id.clone())
                .collect();
        }
        pool.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        pool
    }

    /// Awards `amount` XP to the player, growing stats and fully healing on
    /// any level-up gained, then awards every current party member half as
    /// much (see `award_party_xp`) — fighting beside you pays off even on
    /// rounds where only the player's hit actually lands. Silently does
    /// nothing for the player if they're somehow missing an `Experience`
    /// component (shouldn't happen in practice).
    pub(crate) fn award_player_xp(&mut self, player: Entity, amount: u32) {
        // `XpBoost` is `FieldScope::Run`, so it reads off the player
        // regardless of whether `player` here is the player themself (the
        // only caller today, but the parameter doesn't guarantee it).
        let xp_boost_pct = self.field_buff_power(self.player_entity(), FieldBuffKind::XpBoost);
        let (levels, new_level) = {
            let mut query = self.world.query::<(&mut Experience, &mut Stats)>();
            let Ok((mut exp, mut stats)) = query.get_mut(&mut self.world, player) else {
                return;
            };
            let levels = progression::add_xp(
                &mut exp,
                &mut stats,
                amount,
                crate::tuning::BASELINE_GROWTH_MULTIPLIER,
                // The player has no level ceiling — only creatures do.
                None,
                xp_boost_pct,
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
            self.log_kind(MessageKind::Outcome, format!("You gain {amount} XP."));
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
        let xp_boost_pct = self.field_buff_power(self.player_entity(), FieldBuffKind::XpBoost);
        let party = self.world.resource::<Party>().0.clone();
        for companion in party {
            let species_growth = self
                .world
                .get::<Creature>(companion)
                .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
                .map(|s| s.growth_multiplier)
                .unwrap_or(crate::tuning::BASELINE_GROWTH_MULTIPLIER);
            let individual_roll = self
                .world
                .get::<Potential>(companion)
                .map(|p| p.growth_roll)
                .unwrap_or(Potential::NEUTRAL.growth_roll);
            let growth_multiplier = species_growth * individual_roll;
            let before_level = self
                .world
                .get::<Experience>(companion)
                .map(|e| e.level)
                .unwrap_or(1);
            {
                let mut query = self.world.query::<(&mut Experience, &mut Stats)>();
                let Ok((mut exp, mut stats)) = query.get_mut(&mut self.world, companion) else {
                    continue;
                };
                progression::add_xp(
                    &mut exp,
                    &mut stats,
                    amount,
                    growth_multiplier,
                    Some(crate::tuning::CREATURE_MAX_LEVEL),
                    xp_boost_pct,
                );
            }
            let level = self
                .world
                .get::<Experience>(companion)
                .map(|e| e.level)
                .unwrap_or(before_level);
            if level > before_level {
                let name = self.creature_label(companion);
                self.log_kind(
                    MessageKind::LevelUp,
                    format!("{name} gains {amount} XP and levels up to {level}!"),
                );
                self.install_unlocked_routines(companion, before_level, level);
            }
        }
    }

    /// One decompile attempt against `group`'s front program: spends a
    /// catalyst, rolls `taming::capture_chance`, and on success converts the
    /// target into a tamed program and drops it from the group. Returns
    /// whether that ended the battle.
    ///
    /// The roster-full refusal lives in `ability_unavailable` alone now: a
    /// greyed row can't be planned, and `battle_set_action` refuses one that
    /// somehow is, and nothing inside a resolving round grows `pet_count`
    /// except a successful decompile itself, so that state can't reach here.
    ///
    /// The no-catalyst guard below stays, though: `ability_unavailable`
    /// checks it per slot at *plan* time, but the catalyst is a round-wide
    /// pool, not a per-slot one — two party members can each plan Decompile
    /// while only one catalyst is held, both pass the per-slot check, and the
    /// first to resolve spends the only copy. Without this guard the second
    /// would hit an `expect` instead of a refusal.
    pub(crate) fn attempt_decompile(&mut self, group: usize, player: Entity) -> bool {
        let Some((catalyst, potency)) = self.taming_catalyst() else {
            self.log_kind(
                MessageKind::Outcome,
                "No taming catalyst left — the decompile attempt fizzles.",
            );
            return false;
        };
        let Some(front) = self.front_of_group(group) else {
            return false;
        };
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
            .unwrap_or(DEFAULT_TAMING_DIFFICULTY);
        let bonuses = self.player_decompiler_bonuses();
        let chance = taming::capture_chance(hp_fraction, potency, taming_difficulty, bonuses);
        let roll = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(chance as f64)
        };

        if !roll {
            self.log_kind(
                MessageKind::Outcome,
                "The program's ICE holds — decompile failed!",
            );
            return false;
        }

        let wild_max_hp = self.world.get::<Stats>(front).unwrap().max_hp;
        let nest = self.world.get::<NestGuardian>(front).map(|g| g.nest);
        self.world
            .entity_mut(front)
            .remove::<(Hostile, WanderAi, NestGuardian, Pursuing)>();
        // Battle-scoped state has to be cleared here rather than left to
        // `end_battle`/`clear_battle_status_effects`: `front` is about to
        // leave its group below, so if other groups are still standing the
        // fight goes on without it ever reaching that teardown, and a
        // mirrored buff or a routine's own cooldown would otherwise ride
        // into the roster and never tick again.
        if let Some(mut s) = self.world.get_mut::<StatusEffects>(front) {
            s.active = None;
        }
        if let Some(mut b) = self.world.get_mut::<CombatBuff>(front) {
            b.active = None;
        }
        if let Some(mut c) = self.world.get_mut::<AbilityCooldowns>(front) {
            c.0.clear();
        }
        self.world
            .entity_mut(front)
            .insert((Tamed { owner: player }, Experience::default()));
        self.install_innate_routines(front);
        if let Some(nest) = nest
            && let Some(mut n) = self.world.get_mut::<Nest>(nest)
        {
            n.pending_respawns.push(NEST_RESPAWN_TICKS);
        }
        self.log_kind(
            MessageKind::Outcome,
            "ICE breached! The program now runs under your control.",
        );
        self.award_player_xp(player, wild_max_hp as u32);
        if self.remove_member(group, 0) {
            self.end_battle(player, Some(front));
            return true;
        }
        self.log("Another rogue program from the pack engages!");
        false
    }
}
