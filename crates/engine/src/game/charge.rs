//! Charge routines: a `Damage` routine that winds up over several turns and
//! lands as one hit scaled by how long it was held (`AbilityDef::charge`).
//!
//! The lifecycle is shared by both combat models; only the aim differs. The
//! group model locks the target group's members (`ChargeAim::Group`), the
//! battle map locks board cells (`ChargeAim::Cells`).
//!
//! **`end_charge` is the one place `Charging` is removed, and the one place
//! the cooldown is armed.** Starting a charge pays Power but not the
//! cooldown: an armed-at-start cooldown would tick down during the wind-up,
//! and the routine would be ready again the moment it landed.

use crate::battle::{
    ActionKind, ActionOption, BattleAction, Combatant, DamageRange, Swing, TargetSpec,
};
use crate::tactical::TacticalBattle;
use crate::tuning::ENEMY_ROUTINE_MIN_COOLDOWN;
use crate::*;

/// What a charger does with its turn once it is not yet at full charge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChargeChoice {
    /// Keep winding up: k += 1.
    Hold,
    /// Fire now at k/N of full power.
    Release,
    /// Give the charge up. Power stays spent and the cooldown arms.
    Cancel,
}

/// One body a charge is aimed at, as the AI's kill prediction reads it: the
/// attacker's full-power profile against this defender's.
#[derive(Clone, Copy, Debug)]
pub struct AimedVictim {
    pub hp: i32,
    /// The charger's profile at **full** power; `charge_choice` scales the
    /// band to k/N.
    pub attacker: Combatant,
    pub defender: Combatant,
    /// Percentage points, then the flat deflection, as
    /// `battle::damage_after_mitigation` takes them.
    pub mitigation: i32,
    pub deflection: i32,
}

/// The AI's policy for a charger at `k` of `n` turns: release when the aim
/// holds a body the current k/N hit would kill, cancel when it holds nobody
/// to hit, otherwise hold. Never asked at `k == n`, where the routine fires
/// on its own.
///
/// Kill prediction is `battle::expected_damage` against each victim, so it
/// agrees with every other forecast the game makes.
pub fn charge_choice(k: u32, n: u32, aimed: &[AimedVictim]) -> ChargeChoice {
    if aimed.is_empty() {
        return ChargeChoice::Cancel;
    }
    let kills = aimed.iter().any(|victim| {
        let range = DamageRange {
            min: abilities::charge_scaled(victim.attacker.range.min, k, n),
            max: abilities::charge_scaled(victim.attacker.range.max, k, n),
        };
        let attacker = Combatant {
            range,
            ..victim.attacker
        };
        let expected = battle::expected_damage(attacker, victim.defender).round() as i32;
        battle::damage_after_mitigation(expected, victim.mitigation, victim.deflection) >= victim.hp
    });
    if kills {
        ChargeChoice::Release
    } else {
        ChargeChoice::Hold
    }
}

impl Game {
    /// Begins `entity`'s charge of `ability` at k = 1. The caller has already
    /// paid Power (`spend_power`), and the turn it was started on ends here:
    /// nothing lands on the start turn, so every target gets a turn to react.
    pub(crate) fn start_charge(&mut self, entity: Entity, ability: &AbilityDef, aim: ChargeAim) {
        let Some(spec) = ability.charge else {
            return;
        };
        self.world.entity_mut(entity).insert(Charging {
            ability: ability.id.clone(),
            rounds: spec.rounds,
            progress: 1,
            aim,
        });
        let name = self.charge_name(entity);
        let kind = self.charge_log_kind(entity);
        self.log_kind(
            kind,
            format!("{name} winds up {} (1/{}).", ability.name, spec.rounds),
        );
    }

    /// Removes `entity`'s `Charging` and arms the routine's cooldown, through
    /// the model's own helper (hostiles keep their floor). Both firing and
    /// cancelling end here.
    ///
    /// Called before the effect resolves, for `arm_cooldown`'s reason: a
    /// killing blow ends the battle and wipes battle-scoped components.
    ///
    /// A battle map arms through `arm_tactical_cooldown` with the same floor
    /// a hostile's ordinary routine gets; the group model through its own
    /// helpers. Exactly one runs, whichever fight is open.
    pub(crate) fn end_charge(&mut self, entity: Entity) {
        let Some(charging) = self.world.entity_mut(entity).take::<Charging>() else {
            return;
        };
        let Some(def) = self
            .world
            .resource::<AbilityDb>()
            .get(&charging.ability)
            .cloned()
        else {
            return;
        };
        if self.world.get_resource::<TacticalBattle>().is_some() {
            let floor = if self.is_hostile(entity) {
                ENEMY_ROUTINE_MIN_COOLDOWN
            } else {
                0
            };
            self.arm_tactical_cooldown(entity, &def, floor);
        } else if self.is_hostile(entity) {
            self.arm_enemy_cooldown(entity, &def);
        } else {
            self.arm_cooldown(entity, &def);
        }
    }

    /// One more turn of winding up: k += 1, logged.
    pub(crate) fn hold_charge(&mut self, entity: Entity) {
        let Some(mut charging) = self.world.get_mut::<Charging>(entity) else {
            return;
        };
        charging.progress += 1;
        let (k, n) = (charging.progress, charging.rounds);
        let name = self.charge_name(entity);
        let kind = self.charge_log_kind(entity);
        self.log_kind(kind, format!("{name} holds the charge ({k}/{n})."));
    }

    /// The line a release writes, shared by both models.
    pub(crate) fn log_charge_release(&mut self, entity: Entity, def: &AbilityDef, k: u32, n: u32) {
        let kind = self.charge_log_kind(entity);
        let line = format!(
            "{} releases {} at {k}/{n}.",
            self.charge_name(entity),
            def.name
        );
        self.log_kind(kind, line);
    }

    /// Breaks `entity`'s charge: logged, Power stays spent, cooldown arms.
    /// A no-op on a body that is not charging.
    pub(crate) fn cancel_charge(&mut self, entity: Entity) {
        let Some(charging) = self.world.get::<Charging>(entity) else {
            return;
        };
        let routine = self
            .world
            .resource::<AbilityDb>()
            .get(&charging.ability)
            .map(|d| d.name.clone())
            .unwrap_or_default();
        let name = self.charge_name(entity);
        let kind = self.charge_log_kind(entity);
        self.log_kind(kind, format!("{name}'s {routine} charge is broken."));
        self.end_charge(entity);
    }

    /// Cancels every charge on the party's side: a jack-out attempt, whether
    /// or not it gets clear.
    pub(crate) fn cancel_party_charges(&mut self) {
        let party = self.living_party();
        for entity in party {
            self.cancel_charge(entity);
        }
    }

    /// Fires `entity`'s charge at k/N of full power through `use_ability`,
    /// the ordinary damage path, and ends the charge first. If the aim is
    /// gone the hit retargets: the front group, or a fresh aggro roll for a
    /// hostile.
    pub(crate) fn fire_charge(&mut self, entity: Entity, player: Entity) {
        let Some(charging) = self.world.get::<Charging>(entity).cloned() else {
            return;
        };
        let Some(def) = self
            .world
            .resource::<AbilityDb>()
            .get(&charging.ability)
            .cloned()
        else {
            self.end_charge(entity);
            return;
        };
        let mut bodies = self.charge_bodies(entity, &charging.aim, &def);
        if bodies.is_empty() {
            let aim = self.retargeted_aim(entity, &def, player);
            bodies = self.charge_bodies(entity, &aim, &def);
        }
        let scaled = def.charged(charging.progress, charging.rounds);
        self.end_charge(entity);
        let name = self.creature_label(entity);
        self.log_charge_release(entity, &def, charging.progress, charging.rounds);
        self.use_ability(&scaled, entity, &name, &bodies);
        self.reap_dead_members(player);
    }

    /// One turn of a charger in the group model, run at its place in the
    /// initiative order. Fires on its own at full charge; otherwise `planned`
    /// (the slot's HOLD/RELEASE) decides, and anything else, or no plan at
    /// all, is left to the AI's `charge_choice` — an `[A]ll attack` or
    /// `[R]esolve` must not strand a charging slot.
    pub(crate) fn charge_turn(
        &mut self,
        entity: Entity,
        planned: Option<&BattleAction>,
        player: Entity,
    ) {
        let Some(charging) = self.world.get::<Charging>(entity).cloned() else {
            return;
        };
        let choice = if charging.progress >= charging.rounds {
            ChargeChoice::Release
        } else {
            match planned {
                Some(BattleAction::ChargeHold) => ChargeChoice::Hold,
                Some(BattleAction::ChargeRelease) => ChargeChoice::Release,
                _ => self.charge_ai_choice(entity, &charging),
            }
        };
        match choice {
            ChargeChoice::Hold => self.hold_charge(entity),
            ChargeChoice::Release => self.fire_charge(entity, player),
            ChargeChoice::Cancel => self.cancel_charge(entity),
        }
    }

    /// `charge_choice` for `entity`, with its aimed victims read off the
    /// world. Shared by hostiles, summons and party slots under `[A]`/`[R]`.
    pub(crate) fn charge_ai_choice(&self, entity: Entity, charging: &Charging) -> ChargeChoice {
        let Some(def) = self.world.resource::<AbilityDb>().get(&charging.ability) else {
            return ChargeChoice::Cancel;
        };
        let AbilityEffect::Damage { power, spread, .. } = def.effect else {
            return ChargeChoice::Cancel;
        };
        let level = self.ability_user_level(entity);
        let affinity = self.ability_affinity(entity, &def.effect);
        let band = abilities::scaled_range(DamageRange::centred(power, spread), level, affinity);
        let swing = Swing {
            free: false,
            range: band,
            accuracy: def.accuracy,
            cover_ignored: def.tactical_shape().ignores_cover(),
        };
        let attacker = self.combatant_profile(entity, swing);
        let aimed: Vec<AimedVictim> = self
            .charge_bodies(entity, &charging.aim, def)
            .into_iter()
            .map(|victim| AimedVictim {
                hp: self.world.get::<Stats>(victim).map_or(0, |s| s.hp),
                attacker,
                defender: self.defender_profile_against(
                    entity,
                    victim,
                    Swing::plain(self.natural_range_of(victim)),
                ),
                mitigation: self.effective_mitigation(victim),
                deflection: self.gear_bonus(victim).deflection,
            })
            .collect();
        charge_choice(charging.progress, charging.rounds, &aimed)
    }

    /// The living bodies `aim` would land on right now, with no fallback: an
    /// empty answer is "the aim is gone". On the battle map these are the
    /// bodies in the locked cells that `entity` is hostile to — the ones the
    /// AI weighs; the release itself hits whoever stands there.
    pub(crate) fn charge_bodies(
        &self,
        entity: Entity,
        aim: &ChargeAim,
        def: &AbilityDef,
    ) -> Vec<Entity> {
        let members = match aim {
            ChargeAim::Group(members) => members,
            ChargeAim::Cells(cells) => return self.charged_cells_victims(entity, cells),
        };
        if self.is_hostile(entity) {
            return match def.target {
                AbilityTarget::OneEnemyGroupFront => members
                    .iter()
                    .copied()
                    .filter(|&e| self.creature_alive(e))
                    .take(1)
                    .collect(),
                _ => self.living_party(),
            };
        }
        self.party_charge_bodies(members, def)
    }

    fn charged_cells_victims(&self, entity: Entity, cells: &[(i32, i32)]) -> Vec<Entity> {
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return Vec::new();
        };
        let side = self.acts_for_hostiles(entity);
        crate::tactical::reach::recipients_in_cells(battle, entity, cells)
            .into_iter()
            .filter(|&body| self.creature_alive(body) && self.acts_for_hostiles(body) != side)
            .collect()
    }

    /// `charge_bodies` for a party charger: the group still holding any
    /// aimed member, read through the routine's target kind.
    fn party_charge_bodies(&self, members: &[Entity], def: &AbilityDef) -> Vec<Entity> {
        let alive = |e: &Entity| self.creature_alive(*e);
        let Some(battle) = self.world.get_resource::<BattleState>() else {
            return Vec::new();
        };
        let Some(group) = battle
            .groups
            .iter()
            .position(|g| g.members.iter().any(|m| members.contains(m) && alive(m)))
        else {
            return Vec::new();
        };
        match def.target {
            AbilityTarget::OneEnemyGroupFront => self.front_of_group(group).into_iter().collect(),
            AbilityTarget::WholeEnemyGroup => battle.groups[group]
                .members
                .iter()
                .copied()
                .filter(alive)
                .collect(),
            AbilityTarget::AllEnemies => self.all_living_enemies(),
            AbilityTarget::OneAlly | AbilityTarget::WholeParty => Vec::new(),
        }
    }

    /// The aim a charge falls back to when the one it locked is gone.
    fn retargeted_aim(&mut self, entity: Entity, def: &AbilityDef, player: Entity) -> ChargeAim {
        if self.is_hostile(entity) {
            return self.hostile_charge_aim(def, player);
        }
        let front = self
            .world
            .get_resource::<BattleState>()
            .and_then(|b| b.groups.first())
            .map(|g| g.members.clone())
            .unwrap_or_default();
        ChargeAim::Group(front)
    }

    /// A hostile's aim: one aggro-weighted victim for a single-target
    /// routine (`roll_enemy_target`, as `wild_retaliate` rolls it), else the
    /// whole party.
    fn hostile_charge_aim(&mut self, def: &AbilityDef, player: Entity) -> ChargeAim {
        match def.target {
            AbilityTarget::OneEnemyGroupFront => {
                ChargeAim::Group(vec![self.roll_enemy_target(player)])
            }
            _ => ChargeAim::Group(self.living_party()),
        }
    }

    /// The aim a charge started in the group model locks, from the target the
    /// plan named. A party charger locks the whole aimed group's members
    /// (not its front, which can die without the group going); a hostile
    /// locks its victim, or the whole party for an area routine.
    pub(crate) fn group_charge_aim(
        &mut self,
        entity: Entity,
        def: &AbilityDef,
        target: &battle::SpecialTarget,
        player: Entity,
    ) -> ChargeAim {
        if self.is_hostile(entity) {
            return self.hostile_charge_aim(def, player);
        }
        let members = match target {
            battle::SpecialTarget::EnemyGroup { group } => self
                .retarget(*group)
                .and_then(|g| {
                    self.world
                        .get_resource::<BattleState>()
                        .and_then(|b| b.groups.get(g))
                        .map(|grp| grp.members.clone())
                })
                .unwrap_or_default(),
            _ => self.all_living_enemies(),
        };
        ChargeAim::Group(members)
    }

    /// The `charging k/N -> <group>` tag a roster row carries, or `None` for
    /// a body that is not charging.
    pub(crate) fn charge_tag(&self, entity: Entity) -> Option<ChargeTag> {
        let charging = self.world.get::<Charging>(entity)?;
        let target = match &charging.aim {
            ChargeAim::Group(members) if self.is_hostile(entity) => members
                .iter()
                .copied()
                .find(|&m| self.creature_alive(m))
                .map_or_else(|| "the party".to_string(), |m| self.target_label(m)),
            ChargeAim::Group(members) => self
                .world
                .get_resource::<BattleState>()
                .and_then(|b| {
                    b.groups
                        .iter()
                        .position(|g| g.members.iter().any(|m| members.contains(m)))
                })
                .map_or_else(
                    || "?".to_string(),
                    |g| ((b'A' + g as u8) as char).to_string(),
                ),
            ChargeAim::Cells(_) => "marked cells".to_string(),
        };
        Some(ChargeTag {
            k: charging.progress,
            n: charging.rounds,
            target,
        })
    }

    /// The tag a hostile group's row shows: the furthest-along charger among
    /// its members.
    pub(crate) fn group_charge_tag(&self, members: &[Entity]) -> Option<ChargeTag> {
        members
            .iter()
            .filter_map(|&m| self.charge_tag(m))
            .max_by_key(|t| u64::from(t.k) * 1000 / u64::from(t.n.max(1)))
    }

    /// The two rows a charging slot offers instead of its usual menu.
    pub(crate) fn charge_action_options(&self, entity: Entity) -> Vec<ActionOption> {
        let (k, n) = self
            .world
            .get::<Charging>(entity)
            .map_or((0, 1), |c| (c.progress, c.rounds));
        vec![
            ActionOption {
                kind: ActionKind::ChargeHold,
                key: 'h',
                label: "[h]old".to_string(),
                detail: format!("Keep charging ({k}/{n})"),
                target: TargetSpec::None,
                unavailable: None,
            },
            ActionOption {
                kind: ActionKind::ChargeRelease,
                key: 'x',
                label: "[x] release".to_string(),
                detail: format!("Fire now at {k}/{n} power"),
                target: TargetSpec::None,
                unavailable: None,
            },
        ]
    }

    /// Whether `entity` is charging and has reached full charge, so its turn
    /// is the automatic release and nobody is asked for a choice.
    pub(crate) fn charge_is_full(&self, entity: Entity) -> bool {
        self.world
            .get::<Charging>(entity)
            .is_some_and(|c| c.progress >= c.rounds)
    }

    /// How a charger reads in the battle log.
    fn charge_name(&self, entity: Entity) -> String {
        if entity == self.player_entity() {
            "Your process".to_string()
        } else {
            self.creature_label(entity)
        }
    }

    fn charge_log_kind(&self, entity: Entity) -> MessageKind {
        if self.is_hostile(entity) {
            MessageKind::EnemySpecial
        } else {
            MessageKind::PartyDamage
        }
    }

    /// A hostile's cooldown for `ability`, floored at
    /// `ENEMY_ROUTINE_MIN_COOLDOWN` through `abilities::armed_cooldown`.
    /// Shared by `wild_retaliate`'s ordinary routine and `end_charge`.
    pub(crate) fn arm_enemy_cooldown(&mut self, wild: Entity, ability: &AbilityDef) {
        let armed = abilities::armed_cooldown(ability.cooldown, ENEMY_ROUTINE_MIN_COOLDOWN);
        let mut cooldowns = self
            .world
            .get::<AbilityCooldowns>(wild)
            .map(|c| c.0.clone())
            .unwrap_or_default();
        cooldowns.insert(ability.id.clone(), armed);
        self.world
            .entity_mut(wild)
            .insert(AbilityCooldowns(cooldowns));
    }
}
