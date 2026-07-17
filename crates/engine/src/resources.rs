use bevy_ecs::prelude::{Entity, Resource};
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};

#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum DifficultyMode {
    Permadeath,
    Forgiving,
}

#[derive(Resource, Default, Clone, Copy, Serialize, Deserialize)]
pub struct GameClock {
    pub tick: u64,
}

#[derive(Resource)]
pub struct GameRng(pub StdRng);

const MESSAGE_LOG_CAP: usize = 100;

#[derive(Resource, Default)]
pub struct MessageLog {
    pub lines: Vec<String>,
}

impl MessageLog {
    pub fn push(&mut self, line: impl Into<String>) {
        self.lines.push(line.into());
        if self.lines.len() > MESSAGE_LOG_CAP {
            let excess = self.lines.len() - MESSAGE_LOG_CAP;
            self.lines.drain(0..excess);
        }
    }

    pub fn recent(&self, n: usize) -> &[String] {
        let start = self.lines.len().saturating_sub(n);
        &self.lines[start..]
    }
}

#[derive(Resource, Default)]
pub struct GameOver {
    pub reason: Option<String>,
}

/// The single player-controlled entity. Kept as a resource (rather than
/// re-queried with a `With<Player>` filter each time) since lookups happen
/// on almost every action.
#[derive(Resource, Clone, Copy)]
pub struct PlayerEntity(pub Entity);

/// Active turn-based encounter between the player and a wild creature.
/// Removing this resource ends the battle.
#[derive(Resource)]
pub struct BattleState {
    pub player: Entity,
    pub wild_creature: Entity,
    pub log: Vec<String>,
    pub finished: bool,
    pub player_won: bool,
}
