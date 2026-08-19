//! Lowering and raising a creature's HP, and what follows when it reaches
//! zero.
//!
//! `apply_damage` is the only code path in the engine that lowers HP — see
//! its doc comment. Keeping the mitigation it applies, the death line it
//! emits and the reap that clears the fallen out of their group in one file
//! is what makes that claim checkable by reading rather than by grepping.

use crate::*;

impl Game {
    /// The player's own fumble, second person. One rung, one line.
    ///
    /// A rung is narrated even though most of them are also *applied*
    /// elsewhere, because a fumble that changed something silently reads as
    /// the attack simply not happening.
    pub(crate) fn fumble_line_for_player(
        &self,
        move_name: &str,
        rung: battle::FumbleRung,
    ) -> String {
        match rung {
            battle::FumbleRung::Exposed => {
                format!("Your {move_name} overreaches — you're wide open.")
            }
            battle::FumbleRung::Recoil { dmg } => {
                format!("Your {move_name} backfires for {dmg} damage.")
            }
            battle::FumbleRung::Opening { dmg } => {
                format!("Your {move_name} leaves you open, and it counters for {dmg}.")
            }
            battle::FumbleRung::Crash => {
                format!("Your {move_name} hard-faults. You lose the next cycle.")
            }
        }
    }

    /// The third-person twin of `fumble_line_for_player`, for a companion, a
    /// hostile, or an ability's caster.
    ///
    /// Deliberately never possessive. The caster label reaching this is
    /// `actor_label`'s, which is `"Your process"` for the player, and
    /// `"Your process's Fray"` reads badly where `"Your process overreaches
    /// with Fray"` does not.
    pub(crate) fn fumble_line_for_other(
        &self,
        name: &str,
        move_name: &str,
        rung: battle::FumbleRung,
    ) -> String {
        match rung {
            battle::FumbleRung::Exposed => {
                format!("{name} overreaches with {move_name}, and is left wide open.")
            }
            battle::FumbleRung::Recoil { dmg } => {
                format!("{name} backfires {move_name}, taking {dmg} damage.")
            }
            battle::FumbleRung::Opening { dmg } => {
                format!("{name} leaves an opening with {move_name}, and takes {dmg} for it.")
            }
            battle::FumbleRung::Crash => {
                format!("{name} hard-faults on {move_name}, and loses the next cycle.")
            }
        }
    }

    /// `entity`'s side of an attack roll, with `range` as the band it swings
    /// for.
    ///
    /// **The one place accuracy and evasion are resolved from the ECS**, so
    /// the four creature-versus-creature call sites cannot each derive them
    /// differently. `species_base_speed` already has a player arm; gear
    /// accuracy and evasion are read live off `gear_bonus` because, unlike
    /// `atk` and `mitigation`, neither is baked into `Stats`.
    pub(crate) fn combatant_profile(
        &self,
        entity: Entity,
        range: battle::DamageRange,
    ) -> battle::Combatant {
        let gear = self.gear_bonus(entity);
        let level = self.ability_user_level(entity);
        let speed = self.combat_speed(entity);
        let evasion = battle::evasion_of(speed, level, gear.evasion);
        // The Exposed rung's whole cost. Read here rather than folded into
        // `evasion_of` because it is entity state rather than a property of
        // the numbers, and `evasion_of` is what `balance_sim` calls with no
        // ECS to ask.
        let exposed = self
            .world
            .get::<StatusEffects>(entity)
            .and_then(|s| s.active)
            .is_some_and(|a| a.kind == StatusKind::Exposed);
        let evasion = if exposed {
            evasion * (100 - crate::tuning::EXPOSED_EVASION_PERCENT) as f64 / 100.0
        } else {
            evasion
        };
        battle::Combatant {
            accuracy: battle::accuracy_of(speed, level, gear.accuracy),
            evasion,
            atk: self.effective_atk(entity),
            range,
        }
    }

    /// Lands one rung of the fumble ladder on `fumbler`.
    ///
    /// **Rungs replace rather than stack.** `StatusEffects` holds one
    /// condition at a time and both status rungs go through `arm_status`, so
    /// a second fumble clobbers the first rather than compounding it — a
    /// cumulative top rung is a run-ender.
    ///
    /// The Opening rung's damage was already rolled inside
    /// `battle::resolve_attack`, non-recursively, so a fumbled free swing
    /// resolved as a plain miss. All this does is land it, through
    /// `apply_damage` like every other rung that hurts someone — and like
    /// every rung, it hurts the *fumbler*, which is why `_target` is unused.
    pub(crate) fn apply_fumble_rung(
        &mut self,
        fumbler: Entity,
        _target: Entity,
        rung: battle::FumbleRung,
    ) {
        match rung {
            battle::FumbleRung::Exposed => {
                self.arm_status(
                    fumbler,
                    StatusKind::Exposed,
                    crate::tuning::EXPOSED_DURATION_ROUNDS,
                    0,
                );
            }
            battle::FumbleRung::Recoil { dmg } | battle::FumbleRung::Opening { dmg } => {
                if dmg > 0 {
                    self.apply_damage(fumbler, dmg);
                }
            }
            battle::FumbleRung::Crash => {
                self.arm_status(
                    fumbler,
                    StatusKind::Stun,
                    crate::tuning::CRASH_DURATION_ROUNDS,
                    0,
                );
            }
        }
    }

    /// Rolls one creature-versus-creature attack and applies whatever landed
    /// on the defender, returning the outcome so the caller can log it and
    /// branch on it.
    ///
    /// **The miss branch belongs at the call site, not here.** A missed
    /// Drain must still skip its heal and a missed swing must still skip its
    /// status rider, and `apply_damage` — which stays the only path that
    /// damages a creature — has no way to know about either.
    pub(crate) fn resolve_and_apply_attack(
        &mut self,
        attacker: Entity,
        defender: Entity,
        range: battle::DamageRange,
    ) -> battle::AttackOutcome {
        let attacker_profile = self.combatant_profile(attacker, range);
        // The defender's own band, so an Opening rung's riposte — rolled
        // inside `resolve_attack` — deals real damage rather than zero.
        let defender_profile = self.combatant_profile(defender, self.natural_range_of(defender));
        let outcome = {
            let mut rng = self.world.resource_mut::<GameRng>();
            battle::resolve_attack(attacker_profile, defender_profile, &mut rng.0)
        };
        let rolled = outcome.damage_to_defender();
        if rolled <= 0 {
            if let battle::AttackOutcome::Fumble(rung) = outcome {
                self.apply_fumble_rung(attacker, defender, rung);
            }
            return outcome;
        }
        // **The returned outcome carries what *landed*, not what was
        // rolled.** Mitigation is applied inside `apply_damage`, so the two
        // differ for every defender with any at all — and callers use this
        // figure for their log line and, in `Drain`'s case, for the heal.
        // Reporting the pre-mitigation roll would claim damage the target
        // never took, the same trap `restore_hp` returning its landed figure
        // already closes from the other side.
        let landed = self.apply_damage(defender, rolled);
        match outcome {
            battle::AttackOutcome::Hit { .. } => battle::AttackOutcome::Hit { dmg: landed },
            battle::AttackOutcome::Crit { .. } => battle::AttackOutcome::Crit { dmg: landed },
            other => other,
        }
    }

    /// The band `entity` swings for when nobody has told us which move it is
    /// using — its *first* species move through `attack_range`, falling back
    /// to `PLAYER_UNARMED_DAMAGE`.
    ///
    /// Deliberately the first move rather than a rolled one: rolling here
    /// would spend a `GameRng` draw before the band roll and break every
    /// draw-count assertion `resolve_attack` carries.
    pub(crate) fn natural_range_of(&self, entity: Entity) -> battle::DamageRange {
        let natural = self
            .world
            .get::<Creature>(entity)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
            .and_then(|def| def.moves.first().map(|mv| mv.range()))
            .unwrap_or(crate::tuning::PLAYER_UNARMED_DAMAGE);
        self.attack_range(entity, natural)
    }

    /// Applies `dmg` to `target`, cut by `target`'s mitigation and floored
    /// at 0.
    ///
    /// This is the only path that *damages* a creature — every other write
    /// to `Stats::hp` is a heal, one of the two full-heals, or
    /// `needs_tick_system`, which is `With<Player>`. `kill_outright` is the
    /// one other thing that lowers HP, and it shares this one's `lower_hp`
    /// so the death check cannot be missed by either.
    ///
    /// Only party members are announced. A hostile reaching 0 is reported by
    /// `finish_member`, and the player by `difficulty::death_handling_system`.
    ///
    /// Returns how much actually came off, which is not what was asked for
    /// once mitigation is in play — the same reason `restore_hp` returns its
    /// landed figure rather than its requested one. A log line printing the
    /// requested number claims damage the target never took.
    pub(crate) fn apply_damage(&mut self, target: Entity, dmg: i32) -> i32 {
        let dealt = self.mitigate_incoming_damage(target, dmg);
        self.lower_hp(target, dealt);
        dealt
    }

    /// Kills `target` outright, past any mitigation.
    ///
    /// **The one thing in the game that armour cannot answer**, and it is
    /// deliberately spelled as its own verb rather than as a large `dmg`.
    /// Materialising inside solid substrate (`Game::die_in_the_rock`) is not
    /// an attack with a big number on it; there is no defence against it and
    /// no amount of mitigation that should leave the player standing. Once
    /// `Stats::mitigation` reached the damage path, `apply_damage(player,
    /// hp)` stopped being lethal — even the player's innate 2% leaves a
    /// point behind.
    ///
    /// It is a *kill*, not a damage source, which is what keeps it from
    /// becoming a general mitigation bypass: there is no amount to pass, so
    /// nothing can reach for it to make an ordinary hit hurt more.
    pub(crate) fn kill_outright(&mut self, target: Entity) {
        let hp = self
            .world
            .get::<Stats>(target)
            .map(|s| s.hp)
            .unwrap_or_default();
        self.lower_hp(target, hp);
    }

    /// The single write that lowers a creature's HP. Both `apply_damage` and
    /// `kill_outright` funnel through it, so "one place lowers HP" survives
    /// the second door.
    ///
    /// Death is detected here rather than at the call sites for the reason
    /// `apply_damage`'s doc gives.
    fn lower_hp(&mut self, target: Entity, dmg: i32) {
        let killed = {
            let Some(mut stats) = self.world.get_mut::<Stats>(target) else {
                return;
            };
            let was_alive = stats.hp > 0;
            stats.hp = (stats.hp - dmg).max(0);
            was_alive && stats.hp == 0
        };
        if killed && self.world.resource::<Party>().0.contains(&target) {
            self.announce_program_death(target);
        }
    }

    /// Raises `target`'s HP by `amount`, capped at its maximum, and returns
    /// how much actually landed — zero on a full-health target.
    ///
    /// The return value is what battle logs print. Printing the requested
    /// figure instead let a heal claim twenty points on a target with three
    /// to spare, which reads as the heal having been wasted by the game
    /// rather than by the player's timing.
    pub(crate) fn restore_hp(&mut self, target: Entity, amount: i32) -> i32 {
        let Some(mut stats) = self.world.get_mut::<Stats>(target) else {
            return 0;
        };
        let before = stats.hp;
        stats.hp = (stats.hp + amount).min(stats.max_hp);
        stats.hp - before
    }

    /// Cuts `dmg` by `target`'s total mitigation, in percentage points.
    ///
    /// **Everything comes through `effective_mitigation`**, which is the one
    /// door onto that total: innate `Stats::mitigation` (gear already baked
    /// in by `apply_equipment_delta`), an active `CombatBuff::Mitigation`, a
    /// running `FieldBuffKind::Mitigation`, and the player's party bonus —
    /// already summed and already capped. This used to read the field buff
    /// alone, which meant a species' authored toughness and every worn piece
    /// of armour were invisible here.
    ///
    /// Rounds once, in the same expression as the percentage cut, rather
    /// than rounding the reduction and then subtracting it — two roundings
    /// can discard a point the combined operation keeps.
    ///
    /// Floors at 1 so a landed hit stays a hit under heavy mitigation, but
    /// only when there was a hit to protect: `dmg <= 0` (already a miss, or
    /// no mitigation at all) passes through untouched rather than being
    /// raised to 1.
    pub(crate) fn mitigate_incoming_damage(&self, target: Entity, dmg: i32) -> i32 {
        let percent = self.effective_mitigation(target);
        if percent <= 0 || dmg <= 0 {
            return dmg;
        }
        let reduced = (dmg as f32 * (1.0 - percent as f32 / 100.0)).round() as i32;
        reduced.max(1)
    }

    /// The `Outcome` line for a party member killed in battle: what died and
    /// what died with it.
    ///
    /// Emitted the moment its HP reaches 0, while the entity itself lives on
    /// until `end_battle` reaps it — see that method for why the removal has
    /// to wait.
    fn announce_program_death(&mut self, program: Entity) {
        let name = self.creature_label(program);
        let routines: Vec<String> = self
            .extractable_routines(program)
            .into_iter()
            .map(|def| def.name)
            .collect();
        let line = if routines.is_empty() {
            format!("{name} crashes and is deleted for good.")
        } else {
            format!(
                "{name} crashes and is deleted for good, taking {} with it.",
                routines.join(", ")
            )
        };
        self.log_kind(MessageKind::Outcome, line);
    }

    pub(crate) fn creature_alive(&self, e: Entity) -> bool {
        self.world
            .get::<Stats>(e)
            .map(|s| s.hp > 0)
            .unwrap_or(false)
    }

    /// Clears out any group whose front died to a status tick, awarding
    /// loot and XP exactly as a direct kill would. Walks back to front so
    /// removing a group can't shift a later one out from under the loop.
    /// Returns whether that ended the battle.
    pub(crate) fn reap_dead_members(&mut self, player: Entity) -> bool {
        let mut group = self.living_group_count();
        while group > 0 {
            group -= 1;
            let mut index = self
                .world
                .get_resource::<BattleState>()
                .and_then(|b| b.groups.get(group))
                .map(|g| g.members.len())
                .unwrap_or(0);
            while index > 0 {
                index -= 1;
                let alive = self
                    .world
                    .get_resource::<BattleState>()
                    .and_then(|b| b.groups.get(group))
                    .and_then(|g| g.members.get(index))
                    .is_some_and(|&e| self.creature_alive(e));
                if alive {
                    continue;
                }
                if self.finish_member(group, index, player) {
                    return true;
                }
            }
        }
        false
    }
}
