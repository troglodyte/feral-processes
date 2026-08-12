//! What a fight cost, sampled as it happens.
//!
//! Both non-obvious parts of reading an outcome live here rather than in
//! whoever is driving the rounds, so the played fight and the measured one
//! cannot disagree about what they saw: HP is sampled per round and a round
//! that granted a level is skipped, and "won" is read off the opponents
//! rather than off the player.

use super::report::RepRecord;
use crate::battle::EnemyGroup;
use crate::*;

/// Attached to a staged fight, fed one call per resolved round.
pub struct Watch {
    seed: u64,
    player: Entity,
    party: Vec<Entity>,
    opponents: Vec<Entity>,
    composition: Vec<(String, u32)>,
    level: u32,
    hp_fraction: f32,
    rounds: u32,
    transcript: Vec<String>,
}

impl Watch {
    /// `pub(crate)` so only `stage` may build one — a `Watch` holding
    /// entities from a different fight would report on a fight nobody had.
    ///
    /// Takes the groups rather than a flattened member list so that who the
    /// opponents are and what the composition was cannot disagree. Built
    /// before `begin_battle` for the same reason the member list was noted
    /// there: `BattleState` owns the groups afterwards. Nothing read here is
    /// touched by opening the battle.
    pub(crate) fn new(game: &Game, seed: u64, groups: &[EnemyGroup]) -> Self {
        let player = game.player_entity();
        Self {
            seed,
            player,
            party: game.world.resource::<Party>().0.clone(),
            opponents: groups.iter().flat_map(|g| g.members.clone()).collect(),
            composition: groups
                .iter()
                .map(|g| (g.species.clone(), g.members.len() as u32))
                .collect(),
            level: level_of(game, player),
            hp_fraction: hp_fraction_of(game, player),
            rounds: 0,
            transcript: Vec::new(),
        }
    }

    /// Call once after each resolved round.
    pub fn observe(&mut self, game: &Game) {
        self.rounds += 1;
        // A level-up full-heals (`progression::add_xp`), and the kill that
        // ends a fight is usually the one that grants it — so a reading
        // taken after that round says the fight cost nothing. Skipping the
        // sample keeps the last honest one, which is worth a round of
        // damage in accuracy against being off by the whole fight.
        let now = level_of(game, self.player);
        if now == self.level {
            self.hp_fraction = hp_fraction_of(game, self.player);
        }
        self.level = now;
        // After every round, never at the end: `end_battle` calls
        // `retain_outcomes_since_battle`, which deletes the blow-by-blow and
        // keeps only Outcome/Loot/LevelUp/Raid. `MESSAGE_LOG_CAP` is the
        // second reason — a long fight drops lines off the front before it
        // finishes.
        self.transcript
            .extend(game.battle_log().into_iter().map(|line| line.text));
    }

    /// What the fight cost, once it is over.
    pub fn finish(&self, game: &Game) -> RepRecord {
        // Won means the pack is wiped, so the opponents are what has to be
        // counted — never the player, whose Forgiving defeat is rebooted
        // inside the round that lands it. A stalemate leaves the pack
        // standing and so records as a loss, which is what the round count
        // is then for.
        let won = self.opponents.iter().all(|&e| !alive(game, e));
        RepRecord {
            seed: self.seed,
            won,
            rounds: self.rounds,
            // Zero on a loss, matching `balance_sim`, and not merely a
            // convention: a Forgiving defeat reboots the player to a
            // fraction of max HP inside the losing round, so anything read
            // afterwards measures the reboot rather than the fight.
            player_hp_fraction: if won { self.hp_fraction } else { 0.0 },
            companions_downed: self.party.iter().filter(|&&e| !alive(game, e)).count() as u32,
            composition: self.composition.clone(),
            transcript: self.transcript.clone(),
        }
    }

    pub fn rounds(&self) -> u32 {
        self.rounds
    }
}

fn alive(game: &Game, entity: Entity) -> bool {
    game.world.get::<Stats>(entity).is_some_and(|s| s.hp > 0)
}

fn hp_fraction_of(game: &Game, entity: Entity) -> f32 {
    game.world
        .get::<Stats>(entity)
        .map(|s| (s.hp as f32 / s.max_hp.max(1) as f32).clamp(0.0, 1.0))
        .unwrap_or(0.0)
}

fn level_of(game: &Game, entity: Entity) -> u32 {
    game.world
        .get::<Experience>(entity)
        .map(|e| e.level)
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use crate::arena::scenario::{CompanionSpec, OpponentSpec, PlayerSource, Scenario};
    use crate::arena::test_fight;

    fn scenario(level: u32, zone: u32, party: &[(&str, u32)], against: &[(&str, u32)]) -> Scenario {
        Scenario {
            player: PlayerSource::Fresh { level, zone },
            party: party
                .iter()
                .map(|(species, level)| CompanionSpec {
                    species: (*species).into(),
                    level: *level,
                    ..Default::default()
                })
                .collect(),
            opponents: against
                .iter()
                .map(|(species, count)| OpponentSpec {
                    species: (*species).into(),
                    count: *count,
                })
                .collect(),
            ..Scenario::default()
        }
    }

    /// The regression the transcript half exists for. `end_battle` prunes
    /// the log to Outcome/Loot/LevelUp/Raid, so an implementation that read
    /// the log once the fight was over passes every other test here and
    /// returns nothing but results.
    #[test]
    fn a_won_fights_transcript_survives_end_battle() {
        let s = scenario(20, 1, &[("glitch", 8)], &[("sprite", 1)]);
        let record = test_fight(&s, 3);
        assert!(
            record.won,
            "the fixture is meant to be a walkover: {record:?}"
        );
        assert!(
            record
                .transcript
                .iter()
                .any(|line| line.contains("── round 1 ──")),
            "round narration is dropped by `retain_outcomes_since_battle`: {:?}",
            record.transcript
        );
    }

    /// With a rolled encounter every rep fights something different and the
    /// transcript names only the front group, so without this the report
    /// cannot be read at all.
    #[test]
    fn a_rep_records_what_it_fought() {
        let s = scenario(20, 4, &[], &[("glitch", 2), ("sprite", 1)]);
        let record = test_fight(&s, 5);
        assert_eq!(
            record.composition,
            vec![("glitch".to_string(), 2), ("sprite".to_string(), 1)],
            "in formation order, as staged"
        );
    }

    /// The killing blow usually grants the level that heals the player back
    /// to full, so a fraction read after the fight reports a hard-won win as
    /// costing nothing at all.
    #[test]
    fn a_level_up_on_the_killing_blow_does_not_report_the_fight_as_free() {
        let s = scenario(1, 1, &[], &[("sub_process", 1)]);
        let record = test_fight(&s, 1);
        assert!(record.won, "{record:?}");
        assert!(
            record
                .transcript
                .iter()
                .any(|l| l.contains("reach level") || l.contains("XP")),
            "the fixture must actually level up: {:?}",
            record.transcript
        );
        assert!(
            record.player_hp_fraction < 1.0,
            "eight rounds of damage read back as untouched: {record:?}"
        );
    }
}
