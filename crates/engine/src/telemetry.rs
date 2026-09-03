//! What a fight looked like from the inside, as plain data.
//!
//! No `World` and no IO: the engine builds these into
//! `resources::BattleTelemetry` at the five combat seams, and app-core is
//! the only crate that turns one into a line of text. That split is why
//! `serde_json` is app-core's dependency and not the engine's — see
//! `docs/superpowers/archive/specs/2026-08-09-battle-telemetry-design.md`.

use serde::{Deserialize, Serialize};

/// One recorded event. Tagged with `t` so a reader can dispatch on a single
/// field and so a line is interpretable on its own — every variant carries
/// `fight`, and nothing depends on the line above it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Record {
    FightStart {
        fight: u64,
        seed: u64,
        zone: u32,
        depth: u32,
        party: Vec<PartyMember>,
        enemies: Vec<EnemyGroupInfo>,
    },
    Round {
        fight: u64,
        round: u32,
        party_hp: Vec<i32>,
        enemies: Vec<EnemyGroupHp>,
    },
    EnemyChoice {
        fight: u64,
        round: u32,
        group: usize,
        actor: String,
        /// `move` is a Rust keyword, so the field is spelled out here and
        /// renamed on the wire — the JSON key is what an analysis script
        /// reads and it must stay `move`.
        #[serde(rename = "move")]
        move_name: String,
        target_slot: usize,
        target: String,
        target_hp_before: i32,
        target_max_hp: i32,
        target_bracing: bool,
    },
    PartyAction {
        fight: u64,
        round: u32,
        slot: usize,
        actor: String,
        kind: ActionKind,
        name: Option<String>,
        target_slot: Option<usize>,
    },
    FightEnd {
        fight: u64,
        rounds: u32,
        won: bool,
        player_hp_frac: f32,
        companions_downed: u32,
    },
    /// An extractor finished a cycle. `ok: false` is the fizzle — the only
    /// empirical route to `systems::mining_success_chance` — and `rolled`
    /// against `landed` is what a clog cost.
    Extract {
        tick: u64,
        zone: u32,
        /// The machine's own base-space tile, which is what identifies one
        /// instance from another; `kind` is its `StructureDef` id.
        machine: (i32, i32),
        kind: String,
        tier: u32,
        worker_species: Option<String>,
        item: String,
        rolled: u32,
        landed: u32,
        ok: bool,
    },
    /// An assembler completed one unit, draining its inputs in the same
    /// scope — so consumption and production are one record.
    Assemble {
        tick: u64,
        zone: u32,
        machine: (i32, i32),
        kind: String,
        item: String,
        inputs: Vec<(String, u32)>,
    },
    /// A machine changed status. Transitions only, because
    /// `set_machine_status` already logs only on transition — hanging this
    /// there is what makes the edges free and leaves no duplicate
    /// suppression to write.
    MachineStall {
        tick: u64,
        machine: (i32, i32),
        kind: String,
        status: String,
    },
    /// The player finished a hand-compile. `ticks_spent` against the
    /// machine cycle for the same item is the whole of B2.
    HandCraft {
        tick: u64,
        item: String,
        qty: u32,
        careful: bool,
        bench: Option<String>,
        ticks_spent: u32,
    },
}

impl Record {
    /// Re-keys a record onto another fight. `arena::run` is the one caller:
    /// it builds a fresh `Game` per rep, and a fresh `Game` mints its ids
    /// from 1, so without this every fight in a 200-rep evaluation would
    /// land in the file as fight 1.
    ///
    /// Or-patterns rather than an arm each, and **no wildcard**: a new
    /// variant fails to compile until it is classified as one that carries
    /// a fight or one that does not. That is what keeps "every battle
    /// record carries a fight id" true rather than merely documented.
    ///
    /// The base records are the ones that do not. They are keyed to `tick`
    /// and happen while no fight is open at all, so re-keying one would be
    /// inventing an association rather than correcting it.
    pub(crate) fn set_fight(&mut self, fight: u64) {
        let slot = match self {
            Record::FightStart { fight, .. }
            | Record::Round { fight, .. }
            | Record::EnemyChoice { fight, .. }
            | Record::PartyAction { fight, .. }
            | Record::FightEnd { fight, .. } => fight,
            Record::Extract { .. }
            | Record::Assemble { .. }
            | Record::MachineStall { .. }
            | Record::HandCraft { .. } => return,
        };
        *slot = fight;
    }
}

/// Which `BattleAction` a party slot spent its turn on. Taken from the
/// variant rather than re-derived, so "did the human actually brace or
/// heal" is answerable at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    Attack,
    Special,
    Defend,
    Item,
    Flee,
}

/// A party slot as the fight opened. `species` is `None` for the player,
/// who has no `Creature` component.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartyMember {
    pub slot: usize,
    pub label: String,
    pub species: Option<String>,
    pub level: u32,
    pub max_hp: i32,
}

/// One enemy group as the fight opened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnemyGroupInfo {
    pub group: usize,
    pub species: String,
    pub count: usize,
}

/// One enemy group's living members' HP at the top of a round.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnemyGroupHp {
    pub group: usize,
    pub hp: Vec<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Deserialize` exists for this test and for a future analysis script
    /// reading the file back — it is not dead code to be tidied away.
    ///
    /// The round trip is through `ron` rather than `serde_json` on purpose:
    /// the engine must not grow that dependency, not even as a
    /// dev-dependency for test convenience. It proves the derives are wired,
    /// which is all this crate can honestly claim. The properties that
    /// actually matter are JSON ones — that the tag key is `t`, that a
    /// record is one line — and they are asserted in app-core's
    /// `each_record_is_one_json_line` against the real written file.
    #[test]
    fn every_record_kind_round_trips() {
        let records = vec![
            Record::FightStart {
                fight: 1,
                seed: 904,
                zone: 3,
                depth: 0,
                party: vec![
                    PartyMember {
                        slot: 0,
                        label: "You".to_string(),
                        species: None,
                        level: 12,
                        max_hp: 140,
                    },
                    PartyMember {
                        slot: 1,
                        label: "Sentinel".to_string(),
                        species: Some("sentinel".to_string()),
                        level: 12,
                        max_hp: 180,
                    },
                ],
                enemies: vec![EnemyGroupInfo {
                    group: 0,
                    species: "trojan".to_string(),
                    count: 4,
                }],
            },
            Record::Round {
                fight: 1,
                round: 3,
                party_hp: vec![120, 180],
                enemies: vec![EnemyGroupHp {
                    group: 0,
                    hp: vec![61, 61, 12],
                }],
            },
            Record::EnemyChoice {
                fight: 1,
                round: 3,
                group: 1,
                actor: "trojan".to_string(),
                move_name: "Payload Drop".to_string(),
                target_slot: 2,
                target: "Sprite".to_string(),
                target_hp_before: 44,
                target_max_hp: 90,
                target_bracing: false,
            },
            Record::PartyAction {
                fight: 1,
                round: 3,
                slot: 1,
                actor: "Sentinel".to_string(),
                kind: ActionKind::Special,
                name: Some("redundancy_sync".to_string()),
                target_slot: Some(2),
            },
            Record::FightEnd {
                fight: 1,
                rounds: 10,
                won: false,
                player_hp_frac: 0.0,
                companions_downed: 2,
            },
        ];

        for record in records {
            let text = ron::to_string(&record).expect("record serializes");
            let back: Record = ron::from_str(&text).expect("record parses back");
            assert_eq!(record, back);
        }
    }
}
