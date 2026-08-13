//! What a won fight pays out: equipment drops, loot, experience, and
//! decompiling a defeated program into a companion.

use crate::progression::StatRow;
use crate::tuning::{DECOMPILE_ATTEMPT_BONUS_CAP, WORK_RESOURCE_DROP};
use crate::tuning::{
    DECOMPILER_SKILL_PER_LEVEL, NEST_RESPAWN_TICKS, PARTY_XP_DIVISOR, PERK_POINTS_PER_LEVEL,
    STACK_BOSS_PORTAL_FRAGMENT_DROP, SURFACE_BOSS_LOOT_BAND_FLOOR_PERCENT, SURFACE_BOSS_LOOT_DROPS,
    SURFACE_BOSS_LOOT_RARITY_FLOOR, SURFACE_BOSS_LOOT_VALUE_PER_ZONE,
};
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

    /// Hands the player one dropped copy of `item`, rolling its rare tier
    /// on the way in — **the one way a copy above `Ordinary` enters the
    /// game.** Returns the copy so the caller can name it in its own log
    /// line, which is the only thing the four callers do differently.
    ///
    /// `floor` is the worst tier this source may pay, and exists for one
    /// caller: `SURFACE_BOSS_LOOT_RARITY_FLOOR`. Everything else passes
    /// `Ordinary` and takes the bare ladder.
    ///
    /// **Crafting, buying and buying back are deliberately not callers.**
    /// A made or purchased copy is always plain, so found gear is
    /// categorically better than made gear — which is the whole of why a
    /// player would go looking rather than shopping, and is asserted as an
    /// absence by `crafted_gear_is_never_rare`, since an omission is
    /// invisible otherwise. It also keeps `ItemDef::value`'s two meanings
    /// intact: no recipe becomes underpriced for its output, so
    /// `no_craftable_item_is_worth_more_than_its_ingredients` still binds.
    ///
    /// A non-equippable item takes the early return and **spends no RNG
    /// draw**. A rare tier is defined by `EquipmentStats::for_rarity`, so a
    /// Core Fragment has nothing for one to scale; rolling anyway would also
    /// shift the shared `GameRng` stream on every material drop in the game,
    /// which is the kind of change that silently rewrites a seeded combat
    /// test three files away.
    pub(crate) fn grant_gear_drop(&mut self, item: ItemId, floor: Rarity) -> GearCopy {
        if self.equipment_of(&item).is_none() {
            self.grant_loot(item.clone(), 1);
            return GearCopy::plain(item);
        }
        let rarity = self.roll_gear_rarity().max(floor);
        let copy = GearCopy {
            item,
            rarity,
            tier: 0,
        };
        self.add_copies(&copy, 1);
        copy
    }

    /// How a dropped copy reads in its loot line: the tier's word, the item
    /// name, and the category tag `item_name_tagged` adds.
    ///
    /// A drop line is the one place an item is named to a player who has not
    /// opened a screen, so it is also the only place a rare tier can be
    /// noticed at the moment it is earned — the row colour is a screen away.
    pub(crate) fn drop_label(&self, copy: &GearCopy) -> String {
        match copy.rarity.label() {
            Some(tier) => format!("{tier} {}", self.item_name_tagged(&copy.item)),
            None => self.item_name_tagged(&copy.item),
        }
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
            if roll {
                let copy = self.grant_gear_drop(item, Rarity::Ordinary);
                self.log_kind(
                    MessageKind::Loot,
                    format!("It also drops a {}!", self.drop_label(&copy)),
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
            let copy = self.grant_gear_drop(item, SURFACE_BOSS_LOOT_RARITY_FLOOR);
            self.log_kind(
                MessageKind::Loot,
                format!("Its crash spills a {}!", self.drop_label(&copy)),
            );
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
        let (gain, new_level, grown) = {
            let mut query = self.world.query::<(&mut Experience, &mut Stats)>();
            let Ok((mut exp, mut stats)) = query.get_mut(&mut self.world, player) else {
                return;
            };
            let gain = progression::add_xp(
                &mut exp,
                &mut stats,
                amount,
                crate::tuning::BASELINE_GROWTH_MULTIPLIER,
                // The player has no level ceiling — only creatures do.
                None,
                xp_boost_pct,
            );
            (gain, exp.level, *stats)
        };
        if gain.levels > 0 {
            // The player's block runs longer than a companion's: a level also
            // pays a Perk Point and a point of Decompiler skill, and neither
            // was announced anywhere before this, so a player could bank
            // points for a run without learning they had any.
            let mut rows = gain.stat_rows(&grown).to_vec();
            if let Some(mut perks) = self.world.get_mut::<Perks>(player) {
                let before = perks.points;
                perks.points += PERK_POINTS_PER_LEVEL * gain.levels;
                rows.push(StatRow::new(
                    "Perk Points",
                    before as i32,
                    perks.points as i32,
                ));
            }
            if let Some(mut decompiler) = self.world.get_mut::<Decompiler>(player) {
                let before = decompiler.skill;
                decompiler.skill += DECOMPILER_SKILL_PER_LEVEL * gain.levels as i32;
                rows.push(StatRow::new("Decompiler", before, decompiler.skill));
            }
            self.log_kind(
                MessageKind::LevelUp,
                format!("You gain {amount} XP and reach level {new_level}!"),
            );
            for line in progression::stat_block(&rows) {
                self.log_kind(MessageKind::LevelUp, line);
            }
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
            let (gain, grown) = {
                let mut query = self.world.query::<(&mut Experience, &mut Stats)>();
                let Ok((mut exp, mut stats)) = query.get_mut(&mut self.world, companion) else {
                    continue;
                };
                let gain = progression::add_xp(
                    &mut exp,
                    &mut stats,
                    amount,
                    growth_multiplier,
                    Some(crate::tuning::CREATURE_MAX_LEVEL),
                    xp_boost_pct,
                );
                (gain, *stats)
            };
            let level = self
                .world
                .get::<Experience>(companion)
                .map(|e| e.level)
                .unwrap_or(before_level);
            if gain.levels > 0 {
                let name = self.creature_label(companion);
                self.log_kind(
                    MessageKind::LevelUp,
                    format!("{name} gains {amount} XP and levels up to {level}!"),
                );
                for line in progression::stat_block(&gain.stat_rows(&grown)) {
                    self.log_kind(MessageKind::LevelUp, line);
                }
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

        let bonuses = self.player_decompiler_bonuses();
        // Read before the increment below, deliberately: the count this
        // rolls against is the count the battle screen has been showing all
        // along, so the odds cell is always exactly what the next attempt
        // gets rather than what the last one got.
        let resistance = self.target_resistance(front).unwrap();
        let chance = taming::capture_chance(potency, resistance, bonuses);
        let roll = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(chance as f64)
        };
        let attempts = {
            let mut battle = self.world.resource_mut::<BattleState>();
            let counter = battle.decompile_attempts.entry(front).or_insert(0);
            *counter += 1;
            *counter
        };

        if !roll {
            // Naming the cap matters: without it a player reads a rising
            // number and keeps feeding catalysts to a wall.
            let fraying = if attempts < DECOMPILE_ATTEMPT_BONUS_CAP {
                " Its defences fray a little."
            } else {
                " Its defences are as frayed as they will get."
            };
            self.log_kind(
                MessageKind::Outcome,
                format!("The program's ICE holds — decompile failed!{fraying}"),
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
