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

/// Which research nodes the player has unlocked (see `research::ResearchDb`).
/// Empty at the start of a run — every node in the tree begins locked.
#[derive(Resource, Default)]
pub struct Research(pub std::collections::HashSet<crate::research::ResearchId>);

const MESSAGE_LOG_CAP: usize = 100;

/// How a log line should be presented — a display hint set by whatever
/// engine code produced the line, not derived from the text itself, so
/// frontends don't need to pattern-match message strings to style them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MessageKind {
    #[default]
    Info,
    Loot,
    LevelUp,
    Raid,
}

#[derive(Resource, Default)]
pub struct MessageLog {
    pub lines: Vec<(MessageKind, String)>,
}

impl MessageLog {
    pub fn push(&mut self, line: impl Into<String>) {
        self.push_kind(MessageKind::Info, line);
    }

    pub fn push_kind(&mut self, kind: MessageKind, line: impl Into<String>) {
        self.lines.push((kind, line.into()));
        if self.lines.len() > MESSAGE_LOG_CAP {
            let excess = self.lines.len() - MESSAGE_LOG_CAP;
            self.lines.drain(0..excess);
        }
    }

    pub fn recent(&self, n: usize) -> &[(MessageKind, String)] {
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

/// Active turn-based encounter between the player and one or more wild
/// creatures spawned together as a pack (see `Game::gather_pack`).
/// `wild_creatures[0]` is always the "front" target — the only one the
/// player can actually attack or decompile — while every other entry is a
/// reinforcement waiting its turn; all of them retaliate each round
/// regardless. Removing this resource ends the battle.
#[derive(Resource)]
pub struct BattleState {
    pub player: Entity,
    pub wild_creatures: Vec<Entity>,
    pub log: Vec<String>,
    pub finished: bool,
    pub player_won: bool,
}

/// The player's active battle party can hold at most this many tamed
/// programs at once.
pub const MAX_PARTY_SIZE: usize = 3;

/// The player's active battle party: up to `MAX_PARTY_SIZE` tamed programs
/// that fight alongside them and can be commanded to attack during an
/// intrusion. Membership is mutually exclusive with an active cronjob
/// `Task` on the same entity — a program is either working a structure or
/// fighting beside the player, never both at once.
#[derive(Resource, Default, Clone)]
pub struct Party(pub Vec<Entity>);

/// Which zone sector the player is currently breached into. Starts at 1
/// (the sector the run begins in); breaching a zone portal increments it.
/// Deeper zones regenerate their terrain from a different seed and spawn
/// wild programs with stats scaled by `stat_multiplier` — there's no way
/// back down once you've breached forward.
#[derive(Resource, Clone, Copy, Serialize, Deserialize)]
pub struct ZoneLevel(pub u32);

impl Default for ZoneLevel {
    fn default() -> Self {
        ZoneLevel(1)
    }
}

impl ZoneLevel {
    /// Flat stat multiplier applied to wild programs spawned in this zone:
    /// doubles with each zone level (level 1 = x1, level 2 = x2, level 3 =
    /// x4, ...).
    pub fn stat_multiplier(self) -> i32 {
        1 << (self.0 - 1)
    }
}

/// Where the player materialized on breaching into the current zone sector
/// (set alongside `ZoneLevel` in `Game::new`/`Game::enter_next_zone`) — the
/// origin wild spawns measure distance from to scale stats further out, on
/// top of `ZoneLevel::stat_multiplier` — see `Game::distance_stat_multiplier`.
#[derive(Resource, Clone, Copy, Serialize, Deserialize)]
pub struct ZoneSpawnPoint {
    pub x: i32,
    pub y: i32,
}
