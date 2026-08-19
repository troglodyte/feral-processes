//! How a wild program decides what to swing and who to swing it at.
//!
//! The arithmetic — the feature vector, the weights and the softmax — is
//! `crate::policy`, which knows nothing about the `World`. This file is the
//! other half: reading battle state into that vector, and the one function
//! `combat_enemy.rs` asks for a decision.

use crate::policy::{self, Feature, Features};
use crate::resources::EnemyPolicy;
use crate::tuning::{ENEMY_POLICY_TEMPERATURE, ENGAGED_GROUPS, MAX_MITIGATION_PERCENT};
use crate::*;

impl Game {
    /// The one place a wild program's swing is decided: which move, and at
    /// whom. `None` means nothing it has reaches from where it is standing
    /// — the caller keeps its "circles beyond reach" line.
    ///
    /// Both the trained policy and the baseline exit through here, the same
    /// "one way in" shape as `Game::enter_frame` and `Game::arrive`. With no
    /// weights installed it is exactly the two rolls this replaced: a
    /// uniform pick over the moves that reach, then `roll_enemy_target`.
    ///
    /// `roll_enemy_target` is therefore not orphaned — that fallback is one
    /// caller and the routine branch in `wild_retaliate` is the other.
    pub(crate) fn choose_wild_action(
        &mut self,
        wild: Entity,
        group: usize,
        player: Entity,
    ) -> Option<(AbilityDef, Entity)> {
        self.choose_wild_action_at(wild, group, player, ENEMY_POLICY_TEMPERATURE)
    }

    /// `choose_wild_action` with the softmax temperature supplied rather
    /// than read from `tuning`. The parameter exists because the constant is
    /// what `a_high_temperature_approaches_the_uniform_baseline` is testing:
    /// a dial-back nobody can vary is a dial-back nobody has checked works.
    pub(crate) fn choose_wild_action_at(
        &mut self,
        wild: Entity,
        group: usize,
        player: Entity,
        temperature: f32,
    ) -> Option<(AbilityDef, Entity)> {
        let moves = self.basic_attacks_that_reach(wild, group);
        if moves.is_empty() {
            return None;
        }

        let Some(weights) = self.world.resource::<EnemyPolicy>().0.clone() else {
            // The baseline, and deliberately in this order: it is the order
            // `wild_retaliate` rolled in before the seam existed, so an
            // uninstalled policy leaves every seeded fight in the suite
            // playing out move-for-move as it did.
            let idx = {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_range(0..moves.len())
            };
            let target = self.roll_enemy_target(player);
            let mv = moves[idx].clone();
            self.record_enemy_choice(wild, group, &mv, target);
            return Some((mv, target));
        };

        let targets = self.living_targets(player);
        if targets.is_empty() {
            return None;
        }

        // Scored first, in one pass, because the RNG is a resource and every
        // feature read borrows the world immutably — the same shape the two
        // rolls this replaced used.
        let mut pairs: Vec<(usize, Entity)> = Vec::with_capacity(moves.len() * targets.len());
        let mut scores: Vec<f32> = Vec::with_capacity(moves.len() * targets.len());
        for (mi, mv) in moves.iter().enumerate() {
            for (slot, target) in &targets {
                // The aggro table enters as a fixed prior with a pinned
                // coefficient of 1.0, through `ln` so that the softmax's
                // `exp` hands the weight back unchanged: an all-zero policy
                // is today's distribution exactly, and a trained one
                // *multiplies* it rather than replacing it. Never a learned
                // feature — see `assets/policies/README.md`.
                let prior =
                    (battle::slot_aggro_weight(*slot, self.is_defending(*target)) as f32).ln();
                scores.push(prior + weights.score(&self.action_features(wild, mv, *target, group)));
                pairs.push((mi, *target));
            }
        }

        let idx = {
            let mut rng = self.world.resource_mut::<GameRng>();
            policy::sample_scored(&scores, temperature, &mut rng.0)
        };
        let (mi, target) = pairs[idx];
        let mv = moves[mi].clone();
        self.record_enemy_choice(wild, group, &mv, target);
        Some((mv, target))
    }

    /// Records the swing that was just decided, **before** the caller applies
    /// its damage. This is the only point at which `target_hp_before` exists
    /// — taken any later it is the HP *after* the hit, which silently
    /// inverts the meaning of the whole dataset while still looking
    /// plausible.
    ///
    /// One definition called from both exits of `choose_wild_action_at`
    /// rather than a copy at each: the baseline and the trained policy must
    /// not be able to disagree about what a swing looks like in the file.
    /// The two `None` exits above deliberately do not call it — nothing
    /// reached, so no swing happened.
    fn record_enemy_choice(&mut self, wild: Entity, group: usize, mv: &AbilityDef, target: Entity) {
        let fight = self.fight_id();
        let round = self.telemetry_round();
        self.record(|g| {
            let stats = g.world.get::<Stats>(target).copied().unwrap_or(Stats {
                hp: 0,
                max_hp: 0,
                atk: 0,
                mitigation: 0,
            });
            crate::telemetry::Record::EnemyChoice {
                fight,
                round,
                group,
                actor: g
                    .world
                    .get::<Creature>(wild)
                    .map(|c| c.species.to_string())
                    .unwrap_or_default(),
                move_name: mv.name.clone(),
                target_slot: g.party_slot_of(target).unwrap_or(0),
                target: g.creature_label(target),
                target_hp_before: stats.hp,
                target_max_hp: stats.max_hp,
                target_bracing: g.is_defending(target),
            }
        });
    }

    /// The moves `wild` can actually bring to bear from `group`. Only the
    /// front `ENGAGED_GROUPS` are close enough to swing; anything further
    /// back has to shoot.
    fn basic_attacks_that_reach(&self, wild: Entity, group: usize) -> Vec<AbilityDef> {
        let Some(species_id) = self.world.get::<Creature>(wild).map(|c| c.species.clone()) else {
            return Vec::new();
        };
        let engaged = group < ENGAGED_GROUPS;
        self.world
            .resource::<SpeciesDb>()
            .get(&species_id)
            .map(|def| def.basic_attacks())
            .unwrap_or_default()
            .into_iter()
            .filter(|a| engaged || a.ranged)
            .collect()
    }

    /// Everyone the other side can hit, each with its aggro slot.
    ///
    /// The slot is the position in `player + Party`, counted **before** the
    /// dead are dropped, exactly as `roll_enemy_target` counts it — a fallen
    /// front-rank companion must not promote the member behind it.
    fn living_targets(&self, player: Entity) -> Vec<(usize, Entity)> {
        let party = self.world.resource::<Party>().0.clone();
        std::iter::once(player)
            .chain(party)
            .enumerate()
            .filter(|(_, e)| self.creature_alive(*e))
            .collect()
    }

    /// Reads one candidate `(move, target)` pair into the feature vector the
    /// weights are trained over. Every value is normalised to roughly
    /// `[0, 1]`, which is what lets one weight vector mean the same thing
    /// across species, zones and party sizes — including a modded roster
    /// nobody trained against.
    fn action_features(
        &self,
        wild: Entity,
        mv: &AbilityDef,
        target: Entity,
        group: usize,
    ) -> Features {
        let mut f = Features::zeroed();
        let wild_stats = self.world.get::<Stats>(wild).copied().unwrap_or(Stats {
            hp: 1,
            max_hp: 1,
            atk: 0,
            mitigation: 0,
        });

        // Relative to this species' own best move rather than to an absolute
        // scale, so "hits hard for what it is" means the same thing for a
        // drone and for a modded boss. `.max(1)` is the degenerate-moveset
        // guard: a species whose every move is power 0 scores them all zero
        // rather than dividing by it.
        let best_power = self
            .world
            .get::<Creature>(wild)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
            .map(|def| {
                def.basic_attacks()
                    .iter()
                    .map(|a| a.attack_parts().0.mean())
                    .fold(0.0f64, f64::max)
            })
            .unwrap_or(0.0);
        let (range, status) = mv.attack_parts();
        // The band's mean, so a move is compared on what it averages rather
        // than on either end — a wide weak move must not read as the
        // hardest-hitting thing the species owns.
        f.set(
            Feature::MovePowerRel,
            (range.mean() / best_power.max(1.0)) as f32,
        );
        f.set(Feature::MoveRanged, mv.ranged as u8 as f32);
        f.set(Feature::MoveHasEffect, status.is_some() as u8 as f32);
        let stun_move = matches!(&status, Some(e) if e.kind == StatusKind::Stun);
        let bleed_move = matches!(&status, Some(e) if e.kind == StatusKind::Bleed);
        f.set(Feature::MoveEffectStun, stun_move as u8 as f32);
        f.set(Feature::MoveEffectBleed, bleed_move as u8 as f32);
        f.set(
            Feature::MoveEffectChance,
            status.as_ref().map(|e| e.chance).unwrap_or(0.0),
        );

        let target_stats = self.world.get::<Stats>(target).copied().unwrap_or(Stats {
            hp: 1,
            max_hp: 1,
            atk: 0,
            mitigation: 0,
        });
        let target_hp_frac = target_stats.hp_fraction();
        f.set(Feature::TargetHpFrac, target_hp_frac);
        f.set(
            Feature::TargetIsPlayer,
            (target == self.player_entity()) as u8 as f32,
        );
        let mitigation = self.effective_mitigation(target);
        // Squashed into [0, 1] against the cap rather than against the
        // attacker's ATK. Mitigation is a percentage now, so the attacker's
        // attack is not the right yardstick for it — `MAX_MITIGATION_PERCENT`
        // is what "as hard to hurt as anything gets" means. The feature is
        // pinned to a coefficient of zero in the shipped weights, so this
        // moves no behaviour; what it must still do is stay inside [0, 1].
        f.set(
            Feature::TargetDefRel,
            (mitigation as f32 / MAX_MITIGATION_PERCENT as f32).clamp(0.0, 1.0),
        );
        let status = self
            .world
            .get::<StatusEffects>(target)
            .and_then(|s| s.active);
        let stunned = matches!(status, Some(a) if a.kind == StatusKind::Stun);
        let bleeding = matches!(status, Some(a) if a.kind == StatusKind::Bleed);
        f.set(Feature::TargetStunned, stunned as u8 as f32);
        f.set(Feature::TargetBleeding, bleeding as u8 as f32);
        f.set(
            Feature::TargetBracing,
            self.is_defending(target) as u8 as f32,
        );

        // The real arithmetic, *called* rather than restated — a copy here
        // would drift from the damage the swing then actually deals. This is
        // a projection, so it takes `expected_damage`'s mean rather than
        // rolling: the policy is choosing a swing, not making one, and a
        // draw here would shift every seeded run's stream by however many
        // options it weighed.
        let attacker = self.combatant_profile(wild, self.attack_range(wild, range));
        let defender = self.combatant_profile(target, battle::DamageRange::default());
        let projected = battle::expected_damage(attacker, defender);
        let dmg = (projected * (1.0 - mitigation as f64 / 100.0)).round() as i32;
        f.set(
            Feature::EstDamageFrac,
            (dmg as f32 / target_stats.hp.max(1) as f32).clamp(0.0, 1.0),
        );
        f.set(Feature::WouldKill, (dmg >= target_stats.hp) as u8 as f32);

        f.set(Feature::SelfHpFrac, wild_stats.hp_fraction());
        f.set(
            Feature::SelfFrontGroup,
            (group < ENGAGED_GROUPS) as u8 as f32,
        );

        // The three interactions are the whole of the nonlinearity a linear
        // model gets here: whether a condition is worth spending, and
        // whether it is being spent on someone who already has it.
        f.set(
            Feature::EffectXTargetHealthy,
            f.get(Feature::MoveHasEffect) * target_hp_frac,
        );
        f.set(
            Feature::StunXNotStunned,
            f.get(Feature::MoveEffectStun) * (1.0 - f.get(Feature::TargetStunned)),
        );
        f.set(
            Feature::BleedXNotBleeding,
            f.get(Feature::MoveEffectBleed) * (1.0 - f.get(Feature::TargetBleeding)),
        );
        f
    }
}
