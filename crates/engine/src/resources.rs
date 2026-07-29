use bevy_ecs::prelude::{Entity, Resource};
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::battle::{BattleAction, EnemyGroup};
use crate::dungeon::{Dir, DungeonLevel};
use crate::items::ItemId;
use crate::structures::StructureId;

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
    /// Marks where one resolved battle round ends and the next begins. The
    /// battle screen's log pane is continuous, so without this the narration
    /// of six rounds reads as one undifferentiated block.
    Round,
    /// A line that is a *result* of a battle rather than narration of it —
    /// the kill, the XP, the decompile verdict, the jack-out. `MessageLog::
    /// retain_outcomes_since_battle` keeps exactly these (plus `Loot` and
    /// `LevelUp`, which already tag themselves) when a battle ends, which is
    /// what stops the blow-by-blow following the player onto the map.
    Outcome,
    /// A party member landing damage on a hostile group.
    PartyDamage,
    /// A hostile program landing a plain damage-only move.
    EnemyAttack,
    /// A hostile program doing something other than a plain swing: a move
    /// that also inflicts a status condition, the line naming that
    /// condition, or a carrier spending its round on an installed routine
    /// (see `Game::wild_routine_ready`).
    EnemySpecial,
}

/// Where a battle's narration begins, as a count of lines ever pushed.
///
/// Deliberately not an index into `MessageLog::lines`: the log drains its
/// oldest entries once past `MESSAGE_LOG_CAP`, so an index would come to
/// point at the wrong line in any battle long enough to overflow it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MessageMark(u64);

#[derive(Resource, Default)]
pub struct MessageLog {
    pub lines: Vec<(MessageKind, String)>,
    /// Lines ever pushed, including those since dropped. Marks are minted
    /// from this, and `pushed - dropped == lines.len()` is the invariant
    /// that converts a mark back into an index — every mutation of `lines`
    /// has to keep it.
    pushed: u64,
    /// Lines dropped off the front by the cap. Counted rather than derived
    /// from `pushed - lines.len()`: `retain_outcomes_since_battle` removes
    /// lines from the middle too, which a derived figure would mistake for
    /// front-drops and slide every mark backwards past its own range.
    dropped: u64,
    /// Where the current — or most recently ended — battle's narration
    /// begins. `None` until the first battle: a run that has never fought
    /// has no narration, and defaulting to mark 0 would instead make the
    /// whole log read as one endless battle.
    ///
    /// Deliberately not cleared when a battle ends: the frontend is still
    /// scrolling that battle's results in after the fact and needs the range
    /// to slice. The next `open_battle` replaces it.
    battle_start: Option<MessageMark>,
    /// Where the current round's narration begins. The pane shows one round
    /// at a time, so a resolved round replaces the last rather than piling
    /// on top of it.
    round_start: Option<MessageMark>,
    /// Bumped every time the pane's range resets — a new round or a new
    /// battle — so a frontend pacing the narration can tell it has a fresh
    /// range to scroll rather than comparing text. A per-*battle* counter
    /// is not enough: consecutive rounds are much the same length, so a
    /// reveal that carried its count across one would find the new range
    /// already covered and show it whole.
    generation: u64,
}

impl MessageLog {
    pub fn push(&mut self, line: impl Into<String>) {
        self.push_kind(MessageKind::Info, line);
    }

    pub fn push_kind(&mut self, kind: MessageKind, line: impl Into<String>) {
        self.lines.push((kind, line.into()));
        self.pushed += 1;
        if self.lines.len() > MESSAGE_LOG_CAP {
            let excess = self.lines.len() - MESSAGE_LOG_CAP;
            self.lines.drain(0..excess);
            self.dropped += excess as u64;
        }
    }

    pub fn recent(&self, n: usize) -> &[(MessageKind, String)] {
        let start = self.lines.len().saturating_sub(n);
        &self.lines[start..]
    }

    /// Opens a new battle's narration range at the next line pushed, and
    /// its first round with it.
    pub fn open_battle(&mut self) {
        self.battle_start = Some(MessageMark(self.pushed));
        self.open_round();
    }

    /// Opens a new round's range at the next line pushed. The pane shows one
    /// round at a time, so this is what clears it between them.
    pub fn open_round(&mut self) {
        self.round_start = Some(MessageMark(self.pushed));
        self.generation += 1;
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Where `mark` sits in `lines` right now. Clamped: once the mark has
    /// been dropped past, every line still held is younger than it, so all
    /// of them belong to the range.
    fn index_of(&self, mark: Option<MessageMark>) -> Option<usize> {
        let mark = mark?;
        let start = mark.0.saturating_sub(self.dropped) as usize;
        Some(start.min(self.lines.len()))
    }

    /// The current round's lines, oldest first — what the battle pane shows.
    /// Empty before the run's first battle.
    pub fn since_round(&self) -> &[(MessageKind, String)] {
        match self.index_of(self.round_start) {
            Some(start) => &self.lines[start..],
            None => &[],
        }
    }

    /// Drops the blow-by-blow from the battle range, keeping what the player
    /// should still be reading once the map is back: the battle's results,
    /// and any world news that landed mid-fight. `Raid` is kept because the
    /// background systems in `systems.rs` and `difficulty.rs` write to this
    /// log directly, so a raid alert can arrive inside a battle's range
    /// without being any part of that battle.
    pub fn retain_outcomes_since_battle(&mut self) {
        let Some(start) = self.index_of(self.battle_start) else {
            return;
        };
        let mut index = 0;
        self.lines.retain(|(kind, _)| {
            let keep = index < start
                || matches!(
                    kind,
                    MessageKind::Outcome
                        | MessageKind::Loot
                        | MessageKind::LevelUp
                        | MessageKind::Raid
                );
            index += 1;
            keep
        });
        // Restores `pushed - dropped == lines.len()`. Without this the lines
        // just removed would read as front-drops and drag every mark back
        // past its own range.
        self.pushed = self.dropped + self.lines.len() as u64;
        // What survived is the results, and they are what the map's pane
        // scrolls in — so the round range has to cover them, not the point
        // the final round happened to start at.
        self.round_start = Some(MessageMark(self.dropped + start as u64));
    }
}

/// How many effects the queue holds before dropping its oldest — a
/// backstop for a frontend that never calls `Game::take_effects`, matching
/// the cap `MessageLog` puts on lines.
pub const EFFECT_QUEUE_CAP: usize = 32;

/// What happened to a structure a raid picked, for frontends that want to
/// show it on the map. `Deflected` covers both no-damage outcomes — the
/// shield network zeroing the damage out, and a cronjob worker fully
/// mitigating it — since neither changes any state a renderer could
/// otherwise observe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectKind {
    Hit,
    Deflected,
    Destroyed,
}

/// A transient "something happened here" cue, in world coordinates so a
/// frontend can keep it pinned to its tile as the player moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisualEffect {
    pub pos: (i32, i32),
    pub kind: EffectKind,
}

/// Effects queued since the last `Game::take_effects`. Deliberately not
/// serialized — a cue mid-flight has nothing to say to a reloaded save.
#[derive(Resource, Default)]
pub struct EffectQueue {
    effects: Vec<VisualEffect>,
}

impl EffectQueue {
    pub fn push(&mut self, pos: (i32, i32), kind: EffectKind) {
        self.effects.push(VisualEffect { pos, kind });
        if self.effects.len() > EFFECT_QUEUE_CAP {
            let excess = self.effects.len() - EFFECT_QUEUE_CAP;
            self.effects.drain(0..excess);
        }
    }

    pub fn take(&mut self) -> Vec<VisualEffect> {
        std::mem::take(&mut self.effects)
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

/// Active turn-based encounter between the player's party and one or more
/// wild species groups (see `battle::EnemyGroup`), partitioned out of the
/// pack `Game::gather_pack` collected. Groups 0 and 1 are engaged and can
/// melee; anything further back needs a move flagged `ranged` to reach the
/// party. Removing this resource ends the battle.
#[derive(Resource)]
pub struct BattleState {
    pub player: Entity,
    pub groups: Vec<EnemyGroup>,
    pub round: u32,
    /// This round's chosen action per party slot — index 0 is the player,
    /// 1.. are party members in roster order. `None` means "not yet
    /// chosen"; a round resolves only once every slot is `Some`.
    pub planned: Vec<Option<BattleAction>>,
    pub finished: bool,
    pub player_won: bool,
}

/// The player's active battle party: up to `MAX_PARTY_SIZE` tamed programs
/// that fight alongside them and can be commanded to attack during an
/// intrusion. Membership is mutually exclusive with an active cronjob
/// `Task` on the same entity — a program is either working a structure or
/// fighting beside the player, never both at once.
#[derive(Resource, Default, Clone)]
pub struct Party(pub Vec<Entity>);

/// Center of the player's base platform — the slab of `Biome::Platform`
/// stamped across `MAX_BUILD_DISTANCE_FROM_HOME` when a Home is deployed.
/// `None` until the run's first Home goes down, which is why the opening
/// minutes of a run scale danger exactly as they did before platforms
/// existed.
///
/// Exists as a resource rather than being looked up from the Home entity
/// because `Game::distance_stat_multiplier` and `Game::max_group_size` take
/// `&self`, while querying for the Home needs `&mut self`.
///
/// Deliberately not serialized: it's reconstructed on load from the Home's
/// own position, which `save::SaveData::structures` already carries.
#[derive(Resource, Default, Clone, Copy)]
pub struct Platform {
    pub center: Option<(i32, i32)>,
}

/// What each trading post has bought off the player and will sell back to
/// them — see `Game::buyback_options` and `Game::buy_back`.
///
/// Keyed by `(structure kind, tile)` rather than by `Entity` because a shelf
/// has to outlive the building standing on it: a raid that levels the Market
/// must not erase what the player sold it, and the tile is the only identity
/// a rebuilt structure shares with the one it replaced. Rebuilding on the
/// same footprint therefore reopens the same store, and two Markets in one
/// zone keep separate shelves. The kind is part of the key so a different
/// structure raised on a dead trader's tile inherits nothing.
///
/// Entries for tiles that no longer hold a trader are kept, not pruned —
/// that is the whole point — and are bounded by the tiles built on in one
/// zone before `Game::enter_next_zone` clears the lot.
///
/// `BTreeMap` rather than `HashMap` so save bytes don't depend on hash
/// order; the inner `Vec` stays in insertion order, which is player-driven
/// and gives the trade screen a stable row order.
#[derive(Resource, Default, Clone)]
pub struct BuybackLedger(pub BTreeMap<ShelfKey, Vec<(ItemId, u32)>>);

/// Which shelf: the trader's kind and the tile it stands on — see
/// `BuybackLedger` for why those two and not an `Entity`.
pub type ShelfKey = (StructureId, (i32, i32));

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
        crate::tuning::ZONE_STAT_GROWTH.pow(self.0 - 1)
    }
}

/// Whether the player is walking the zone map or is down inside a dungeon,
/// and where in it.
///
/// The dungeon coordinates live *here* rather than on the player's
/// `Position` component, and that is the load-bearing decision of the whole
/// dungeon layer. `Position` is the shared coordinate space that structures,
/// wild programs, nests, cronjob targets, raid pathing, the build radius and
/// `Game::view_entities` all live in. Moving the player into dungeon
/// coordinates through it would put them on a surface tile that means
/// something else entirely, and every one of those systems would quietly
/// misbehave.
///
/// So while underground the player's `Position` stays pinned to `entrance`
/// — the surface tile they walked in through. Nothing on the surface has to
/// know the dungeon exists, and the consequences are the right ones for
/// free: the base is where it was left, cronjobs keep paying out, and a raid
/// can land while the player is four levels down.
#[derive(Resource, Clone, Copy, Default, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Locale {
    #[default]
    Surface,
    Dungeon {
        /// 1 at the first level below the surface, counting up as you
        /// descend. Part of the `dungeon::LevelSpec` the level regenerates
        /// from.
        depth: u32,
        /// How many levels this shaft runs before it bottoms out. Carried
        /// rather than recomputed from `entrance` because it is also part
        /// of the level spec — the bottom level is generated without a way
        /// down — and a shaft that changed length underneath the party
        /// would strand them.
        floors: u32,
        x: i32,
        y: i32,
        facing: Dir,
        /// The surface tile of the entrance walked in through — where the
        /// player's `Position` is pinned, and where they pop out on
        /// climbing back up from depth 1.
        entrance: (i32, i32),
    },
}

/// Which level of which shaft a `LevelMemory` belongs to.
///
/// Keyed by the breach's surface tile rather than by anything about the
/// level, because that tile is what makes a shaft itself — it is already
/// half of `dungeon::LevelSpec`. Two breaches in a sector therefore keep
/// separate maps of their separate depth-3s.
pub type LevelKey = ((i32, i32), u32);

/// What the party learned about one dungeon level by walking it.
///
/// This is the only dungeon state that is saved rather than regenerated.
/// The level itself is a pure function of its `dungeon::LevelSpec`, but what
/// the player has *seen* of it is not — that is the run's history, and
/// losing it on load would hand back a blank map of a level already walked.
///
/// `BTreeSet` rather than `HashSet` so the encoded save bytes don't depend
/// on hash order.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct LevelMemory {
    /// Every cell the party has had in view — see `Game::view_cone`, which
    /// both this and the first-person view are filled from.
    pub seen: BTreeSet<(i32, i32)>,
    /// Cells whose cache has been emptied.
    pub looted: BTreeSet<(i32, i32)>,
    /// Sealed doors that have been opened, which stay open.
    pub opened: BTreeSet<(i32, i32)>,
    /// Whether this level's lair has been cleared. Only ever true on the
    /// bottom level of a shaft, which is the only level that has one.
    pub cleared: bool,
    /// Where the party was jumped. Kept for the map alone: a corridor that
    /// has cost you something is worth marking, and it is the one landmark
    /// the level's own layout can't tell you about.
    pub fights: BTreeSet<(i32, i32)>,
}

/// Everything the party has learned about every dungeon level in this zone.
///
/// Zone-local, and **not** self-clearing: like `BuybackLedger` this has to
/// be wiped by name in `Game::enter_next_zone`, because breaching does not
/// despawn what a zone accumulated. Left behind, a stale entry would draw
/// the previous sector's walked corridors onto a new sector's map at
/// whatever tile happened to collide.
#[derive(Resource, Clone, Default, Debug, Serialize, Deserialize)]
pub struct DungeonMemory(pub BTreeMap<LevelKey, LevelMemory>);

/// The level the player is currently standing in, or `None` on the surface.
///
/// Deliberately not serialized: it regenerates from `(WorldMap::seed,
/// Locale::depth)` on load, exactly as terrain regenerates from the world
/// seed. See `dungeon::generate`.
#[derive(Resource, Default)]
pub struct CurrentDungeon(pub Option<DungeonLevel>);

/// Where the player materialized on breaching into the current zone sector
/// (set alongside `ZoneLevel` in `Game::new`/`Game::enter_next_zone`) — the
/// origin wild spawns measure distance from to scale stats further out, on
/// top of `ZoneLevel::stat_multiplier` — see `Game::distance_stat_multiplier`.
#[derive(Resource, Clone, Copy, Serialize, Deserialize)]
pub struct ZoneSpawnPoint {
    pub x: i32,
    pub y: i32,
}
